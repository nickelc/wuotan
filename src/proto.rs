use std::convert::TryInto;
use std::fmt;

use tracing::instrument;

mod error;
mod util;

use crate::device::Handle;
use error::Error;
use util::BatchIterator;
use util::HandleExt;

macro_rules! consts {
    ($($(#[$outer:meta])* const $name:ident = $value:expr;)+) => {
        $($(#[$outer])* const $name: [u8; 4] = u32::to_le_bytes($value);)*
    }
}

consts! {
    const CONTROL_TYPE_SESSION = 0x64;
    const CONTROL_TYPE_PIT_FILE = 0x65;
    #[allow(dead_code)]
    const CONTROL_TYPE_FILE_TRANSFER = 0x66;
    const CONTROL_TYPE_END_SESSION= 0x67;

    const SESSION_REQUEST_TYPE_BEGIN_SESSION = 0x00;
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_DEVICE_TYPE = 0x01;
    const SESSION_REQUEST_TYPE_TOTAL_BYTES = 0x02;
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_FILE_PART_SIZE = 0x05;
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_ENABLE_TFLASH = 0x08;

    const END_SESSION_REQUEST_TYPE_END_SESSION = 0x00;
    const END_SESSION_REQUEST_TYPE_REBOOT = 0x01;

    const FILE_REQUEST_TYPE_FLASH = 0x00;
    const FILE_REQUEST_TYPE_PART = 0x02;
    const FILE_REQUEST_TYPE_END_TRANSFER = 0x03;

    const FILE_END_TRANSFER_DEST_PHONE = 0x00;
    const FILE_END_TRANSFER_DEST_MODEM = 0x01;

    const PIT_REQUEST_TYPE_DUMP = 0x01;
    const PIT_REQUEST_TYPE_PART = 0x02;
    const PIT_REQUEST_TYPE_END_TRANSFER = 0x03;

    const RESPONSE_TYPE_SEND_FILE_PART = 0x00;
    const RESPONSE_TYPE_SETUP_SESSION = 0x64;
    const RESPONSE_TYPE_PIT_FILE = 0x65;
    const RESPONSE_TYPE_FILE_TRANSFER = 0x66;
    const RESPONSE_TYPE_END_SESSION = 0x67;
}

#[instrument(skip(handle))]
pub fn handshake(handle: &Handle) -> Result<(), Error> {
    handle.write(b"ODIN")?;

    let mut buf = [0; 8];
    match handle.read(&mut buf)? {
        n if n == 4 && &buf[..4] == b"LOKE" => Ok(()),
        _ => Err(Error::Handshake),
    }
}

#[instrument(skip(handle))]
pub fn begin_session(handle: &Handle) -> Result<u32, Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_SESSION);
    buf[4..8].copy_from_slice(&SESSION_REQUEST_TYPE_BEGIN_SESSION);
    // Odin version
    buf[8..12].copy_from_slice(&4u32.to_le_bytes());

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_SETUP_SESSION);

    let default_packet_size = u32::from_le_bytes((&buf[4..8]).try_into().unwrap());
    tracing::debug!(default_packet_size, "in:  {:X?}", buf);
    Ok(default_packet_size)
}

#[instrument(skip(handle))]
pub fn setup_file_part_size(handle: &Handle, size: u32) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_SESSION);
    buf[4..8].copy_from_slice(&SESSION_REQUEST_TYPE_FILE_PART_SIZE);
    buf[8..12].copy_from_slice(&size.to_le_bytes());

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_SETUP_SESSION);

    let result = u32::from_le_bytes((&buf[4..8]).try_into().unwrap());
    tracing::debug!(result, "in:  {:X?}", buf);
    Ok(())
}

#[instrument(skip(handle))]
pub fn send_total_size(handle: &Handle, size: u64) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_SESSION);
    buf[4..8].copy_from_slice(&SESSION_REQUEST_TYPE_TOTAL_BYTES);
    buf[8..16].copy_from_slice(&size.to_le_bytes());

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_SETUP_SESSION);
    Ok(())
}

#[instrument(skip(handle))]
fn begin_file_transfer(handle: &Handle) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_FILE_TRANSFER);
    buf[4..8].copy_from_slice(&FILE_REQUEST_TYPE_FLASH);

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_FILE_TRANSFER);
    Ok(())
}

#[instrument(skip(handle))]
fn begin_batch_file_transfer(handle: &Handle, size: u32) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_FILE_TRANSFER);
    buf[4..8].copy_from_slice(&FILE_REQUEST_TYPE_PART);
    buf[8..12].copy_from_slice(&size.to_le_bytes());

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_FILE_TRANSFER);
    Ok(())
}

#[instrument(skip(handle, chunk))]
fn send_file_chunk(handle: &Handle, chunk_idx: u32, chunk: &[u8]) -> Result<(), Error> {
    tracing::debug!("out: {:X?}", &chunk[..16]);
    handle.write(chunk)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_SEND_FILE_PART);

    let resp_n = u32::from_le_bytes((&buf[4..8]).try_into().unwrap());
    assert_eq!(chunk_idx, resp_n);
    tracing::debug!(resp_n, "in: {:X?}", &buf);
    Ok(())
}

#[instrument(skip(handle))]
fn end_batch_file_transfer(
    handle: &Handle,
    target: &FileTarget,
    effective_size: u32,
    eof: bool,
) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_FILE_TRANSFER);
    buf[4..8].copy_from_slice(&FILE_REQUEST_TYPE_END_TRANSFER);

    buf[12..16].copy_from_slice(&effective_size.to_le_bytes());
    buf[16..20].copy_from_slice(&u32::to_le_bytes(0)); // unknown1
    match target {
        FileTarget::ApplicationProcessor {
            device_type,
            identifier,
        } => {
            buf[8..12].copy_from_slice(&FILE_END_TRANSFER_DEST_PHONE);

            buf[20..24].copy_from_slice(&device_type.to_le_bytes());
            buf[24..28].copy_from_slice(&identifier.to_le_bytes());
            buf[28..32].copy_from_slice(&u32::from(eof).to_le_bytes());
        }
        FileTarget::CommunicationProcessor { device_type } => {
            buf[8..12].copy_from_slice(&FILE_END_TRANSFER_DEST_MODEM);

            buf[20..24].copy_from_slice(&device_type.to_le_bytes());
            buf[24..28].copy_from_slice(&u32::from(eof).to_le_bytes());
        }
    }
    tracing::debug!("out: {:X?}", &buf[..32]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_FILE_TRANSFER);
    Ok(())
}

pub enum FileTarget {
    ApplicationProcessor { device_type: u32, identifier: u32 },
    CommunicationProcessor { device_type: u32 },
}

impl fmt::Debug for FileTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileTarget::ApplicationProcessor {
                device_type,
                identifier,
            } => f
                .debug_struct("AP")
                .field("device_type", device_type)
                .field("identifier", identifier)
                .finish(),
            FileTarget::CommunicationProcessor { device_type } => f
                .debug_struct("CP")
                .field("device_type", device_type)
                .finish(),
        }
    }
}

use std::io::Read;

#[instrument(skip(handle, file))]
pub fn file_transfer<R: Read>(
    handle: &Handle,
    target: &FileTarget,
    file: &mut R,
    file_size: u64,
) -> Result<(), Error> {
    begin_file_transfer(handle)?;

    let it = BatchIterator::new(file_size, 1024 * 1024, 30);

    for batch in it {
        begin_batch_file_transfer(handle, batch.size())?;
        for n in batch.chunks() {
            let mut buf = vec![0; 1024 * 1024];
            util::fill_buf(file, &mut buf)?;

            handle.with_post_write_op(|handle| send_file_chunk(handle, n, &buf))?;
        }
        end_batch_file_transfer(handle, target, batch.effective_size(), batch.is_last())?;
    }
    Ok(())
}

#[instrument(skip(handle))]
pub fn receive_pit(handle: &Handle) -> Result<Vec<u8>, Error> {
    tracing::debug!("start pit transfer");
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_PIT_FILE);
    buf[4..8].copy_from_slice(&PIT_REQUEST_TYPE_DUMP);

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_PIT_FILE);
    let pit_size = u32::from_le_bytes((&buf[4..8]).try_into().unwrap());
    tracing::debug!(pit_size, "in:  {:X?}", buf);
    let mut pit_buf = vec![0; pit_size as usize];

    let mut req_buf = vec![0; 1024];
    req_buf[0..4].copy_from_slice(&CONTROL_TYPE_PIT_FILE);
    req_buf[4..8].copy_from_slice(&PIT_REQUEST_TYPE_PART);

    handle.with_post_read_op(|handle| {
        let it = pit_buf.chunks_mut(500).enumerate();
        let (count, _) = it.size_hint();
        tracing::debug!(count);
        for (i, res_buf) in it {
            req_buf[8..12].copy_from_slice(&(i as u32).to_le_bytes());

            tracing::debug!("out: {:X?}", &req_buf[..16]);
            handle.write(&req_buf)?;

            let n = handle.read(res_buf)?;
            tracing::debug!(n, "in:  {:X?}", &res_buf[..16]);
        }
        Ok(())
    })?;

    tracing::debug!("end pit transfer");
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_PIT_FILE);
    buf[4..8].copy_from_slice(&PIT_REQUEST_TYPE_END_TRANSFER);

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    tracing::debug!("in:  {:X?}", buf);

    assert_eq!(buf[0..4], RESPONSE_TYPE_PIT_FILE);
    Ok(pit_buf)
}

#[instrument(skip(handle))]
pub fn end_session(handle: &Handle) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_END_SESSION);
    buf[4..8].copy_from_slice(&END_SESSION_REQUEST_TYPE_END_SESSION);

    tracing::debug!("out: {:X?}", &buf[..16]);
    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_END_SESSION);
    Ok(())
}

#[instrument(skip(handle))]
pub fn reboot(handle: &Handle) -> Result<(), Error> {
    let mut buf = vec![0; 1024];
    buf[0..4].copy_from_slice(&CONTROL_TYPE_END_SESSION);
    buf[4..8].copy_from_slice(&END_SESSION_REQUEST_TYPE_REBOOT);

    handle.write(&buf)?;

    let mut buf = vec![0; 8];
    let n = handle.read(&mut buf)?;
    debug_assert_eq!(n, 8);

    assert_eq!(buf[0..4], RESPONSE_TYPE_END_SESSION);
    Ok(())
}
