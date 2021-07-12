use std::fmt;
use std::io::Error as IoError;

use rusb::Error as UsbError;

#[derive(Debug)]
pub enum Error {
    Handshake,
    IoError(IoError),
    UsbError(UsbError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Handshake => f.write_str("handshake failed"),
            Error::IoError(_) => f.write_str("io error"),
            Error::UsbError(_) => f.write_str("usb error"),
        }
    }
}

impl std::error::Error for Error {}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}

impl From<UsbError> for Error {
    fn from(e: UsbError) -> Self {
        Error::UsbError(e)
    }
}
