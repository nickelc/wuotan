use std::fmt;
use std::io::Error as IoError;

use rusb::Error as UsbError;

#[derive(Debug)]
pub enum Error {
    Handshake,
    Io(IoError),
    Usb(UsbError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Handshake => f.write_str("handshake failed"),
            Error::Io(_) => f.write_str("io error"),
            Error::Usb(_) => f.write_str("usb error"),
        }
    }
}

impl std::error::Error for Error {}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::Io(e)
    }
}

impl From<UsbError> for Error {
    fn from(e: UsbError) -> Self {
        Error::Usb(e)
    }
}
