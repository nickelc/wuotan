use std::fmt;
use std::io;
use std::ops::Deref;

use byteorder::{ReadBytesExt, LE};

const PIT_SIGNATURE: u32 = 0x12349876;

#[derive(Debug)]
pub struct Pit {
    _signature: u32,
    pub unknown1: u32,
    pub unknown2: u32,
    pub unknown3: u16,
    pub unknown4: u16,
    pub unknown5: u16,
    pub unknown6: u16,
    pub unknown7: u16,
    pub unknown8: u16,
    pub entries: Vec<Entry>,
}

impl Pit {
    pub fn from_read<R: io::Read>(mut r: R) -> io::Result<Self> {
        let _signature = r.read_u32::<LE>()?;
        if _signature != PIT_SIGNATURE {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let count = r.read_u32::<LE>()?;
        let unknown1 = r.read_u32::<LE>()?;
        let unknown2 = r.read_u32::<LE>()?;
        let unknown3 = r.read_u16::<LE>()?;
        let unknown4 = r.read_u16::<LE>()?;
        let unknown5 = r.read_u16::<LE>()?;
        let unknown6 = r.read_u16::<LE>()?;
        let unknown7 = r.read_u16::<LE>()?;
        let unknown8 = r.read_u16::<LE>()?;

        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            entries.push(Entry::from_read(&mut r)?);
        }
        Ok(Self {
            _signature,
            unknown1,
            unknown2,
            unknown3,
            unknown4,
            unknown5,
            unknown6,
            unknown7,
            unknown8,
            entries,
        })
    }
}

#[derive(Debug)]
pub struct Entry {
    pub binary_type: BinaryType,
    pub device_type: DeviceType,
    pub identifier: u32,
    pub attributes: Attributes,
    pub update_attributes: UpdateAttributes,
    pub blocksize_or_offset: u32,
    pub block_count: u32,
    pub file_offset: u32,
    pub file_size: u32,
    pub partition_name: Name,
    pub flash_filename: Name,
    pub fota_filename: Name,
}

#[derive(Debug)]
pub enum BinaryType {
    ApplicationProcessor,
    CommunicationProcessor,
    Unknown(u32),
}

impl BinaryType {
    pub fn as_u32(&self) -> u32 {
        match self {
            BinaryType::ApplicationProcessor => 0,
            BinaryType::CommunicationProcessor => 1,
            BinaryType::Unknown(val) => *val,
        }
    }
}

impl From<u32> for BinaryType {
    fn from(val: u32) -> Self {
        match val {
            0 => BinaryType::ApplicationProcessor,
            1 => BinaryType::CommunicationProcessor,
            n => BinaryType::Unknown(n),
        }
    }
}

impl fmt::Display for BinaryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryType::ApplicationProcessor => f.write_str("AP"),
            BinaryType::CommunicationProcessor => f.write_str("CP"),
            BinaryType::Unknown(val) => f.write_fmt(format_args!("UNKNOWN({})", val)),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum DeviceType {
    OneNAND,
    File,
    MMC,
    All,
    Unknown(u32),
}

impl DeviceType {
    pub fn as_u32(&self) -> u32 {
        match self {
            DeviceType::OneNAND => 0,
            DeviceType::File => 1,
            DeviceType::MMC => 2,
            DeviceType::All => 3,
            DeviceType::Unknown(val) => *val,
        }
    }
}

impl From<u32> for DeviceType {
    fn from(val: u32) -> Self {
        match val {
            0 => DeviceType::OneNAND,
            1 => DeviceType::File,
            2 => DeviceType::MMC,
            3 => DeviceType::All,
            n => DeviceType::Unknown(n),
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::OneNAND => f.write_str("OneNAND"),
            DeviceType::File => f.write_str("File/FAT"),
            DeviceType::MMC => f.write_str("MMC"),
            DeviceType::All => f.write_str("All (?)"),
            DeviceType::Unknown(val) => f.write_fmt(format_args!("UNKNOWN({})", val)),
        }
    }
}

bitflags::bitflags! {
    pub struct Attributes: u32 {
        const WRITE = 0x0001;
        const STL = 0x0010;
    }
}

bitflags::bitflags! {
    pub struct UpdateAttributes: u32 {
        const FOTA = 0x0001;
        const SECURE = 0x0010;
    }
}

impl Entry {
    pub fn from_read<R: io::Read>(mut r: R) -> io::Result<Entry> {
        let binary_type = r.read_u32::<LE>()?;
        let device_type = r.read_u32::<LE>()?;
        let identifier = r.read_u32::<LE>()?;
        let attributes = r.read_u32::<LE>()?;
        let update_attributes = r.read_u32::<LE>()?;
        let blocksize_or_offset = r.read_u32::<LE>()?;
        let block_count = r.read_u32::<LE>()?;
        let file_offset = r.read_u32::<LE>()?;
        let file_size = r.read_u32::<LE>()?;
        let mut partition_name = Name([0; 32]);
        r.read_exact(&mut partition_name.0)?;
        let mut flash_filename = Name([0; 32]);
        r.read_exact(&mut flash_filename.0)?;
        let mut fota_filename = Name([0; 32]);
        r.read_exact(&mut fota_filename.0)?;
        Ok(Entry {
            binary_type: BinaryType::from(binary_type),
            device_type: DeviceType::from(device_type),
            identifier,
            attributes: unsafe { Attributes::from_bits_unchecked(attributes) },
            update_attributes: unsafe { UpdateAttributes::from_bits_unchecked(update_attributes) },
            blocksize_or_offset,
            block_count,
            file_offset,
            file_size,
            partition_name,
            flash_filename,
            fota_filename,
        })
    }
}

pub struct Name([u8; 32]);

impl Deref for Name {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        let pos = self.0.iter().position(|c| *c == 0).unwrap_or(32);
        &self.0[0..pos]
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:X?}", self.0))
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pos = self.0.iter().position(|c| *c == 0).unwrap_or(32);
        if let Ok(s) = std::str::from_utf8(&self.0[0..pos]) {
            f.write_str(s)
        } else {
            f.write_str("invalid uft8 data")
        }
    }
}
