use std::convert::TryInto;

use tracing::instrument;

mod error;
mod util;

use crate::device::Handle;
use error::Error;
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
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_TOTAL_BYTES = 0x02;
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_FILE_PART_SIZE = 0x05;
    #[allow(dead_code)]
    const SESSION_REQUEST_TYPE_ENABLE_TFLASH = 0x08;

    const END_SESSION_REQUEST_TYPE_END_SESSION = 0x00;
    #[allow(dead_code)]
    const END_SESSION_REQUEST_TYPE_REBOOT = 0x01;

    #[allow(dead_code)]
    const PIT_REQUEST_TYPE_FLASH = 0x00;
    const PIT_REQUEST_TYPE_DUMP = 0x01;
    const PIT_REQUEST_TYPE_PART = 0x02;
    const PIT_REQUEST_TYPE_END_TRANSFER = 0x03;

    #[allow(dead_code)]
    const RESPONSE_TYPE_SEND_FILE_PART = 0x00;
    const RESPONSE_TYPE_SETUP_SESSION = 0x64;
    const RESPONSE_TYPE_PIT_FILE = 0x65;
    #[allow(dead_code)]
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
