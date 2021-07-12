use std::{io, slice::Iter, time::Duration, vec::IntoIter};

use rusb::constants::LIBUSB_CLASS_DATA;
use rusb::{Error, UsbContext};

const VENDOR_ID: u16 = 0x04E8;
const PRODUCT_IDS: [u16; 3] = [0x6601, 0x685D, 0x68C3];

#[derive(Debug)]
pub struct Devices(Vec<Device>);

impl Devices {
    pub fn iter(&self) -> Iter<Device> {
        self.0.iter()
    }
}

impl IntoIterator for Devices {
    type Item = Device;
    type IntoIter = IntoIter<Device>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug)]
pub struct Device {
    device: rusb::Device<rusb::Context>,
    iface_number: u8,
    #[allow(dead_code)]
    alt_setting: u8,
    read_endpoint: u8,
    write_endpoint: u8,
}

impl Device {
    pub fn open(&self, timeout: Duration) -> Result<Handle, Error> {
        let handle = self.device.open()?;

        Ok(Handle {
            handle,
            timeout,
            iface_number: self.iface_number,
            alt_setting: self.alt_setting,
            read_endpoint: self.read_endpoint,
            write_endpoint: self.write_endpoint,
        })
    }

    pub fn id(&self) -> Result<(u16, u16), Error> {
        let dd = self.device.device_descriptor()?;
        Ok((dd.vendor_id(), dd.product_id()))
    }

    pub fn bus_number(&self) -> u8 {
        self.device.bus_number()
    }

    pub fn address(&self) -> u8 {
        self.device.address()
    }
}

pub struct Handle {
    handle: rusb::DeviceHandle<rusb::Context>,
    timeout: Duration,
    iface_number: u8,
    #[allow(dead_code)]
    alt_setting: u8,
    read_endpoint: u8,
    write_endpoint: u8,
}

impl Handle {
    #[allow(dead_code)]
    pub fn device(&self) -> Device {
        Device {
            device: self.handle.device(),
            iface_number: self.iface_number,
            alt_setting: self.alt_setting,
            read_endpoint: self.read_endpoint,
            write_endpoint: self.write_endpoint,
        }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.handle.reset()
    }

    pub fn claim(&mut self) -> Result<(), Error> {
        self.handle.claim_interface(self.iface_number)
    }

    pub fn release(&mut self) -> Result<(), Error> {
        self.handle.release_interface(self.iface_number)
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        self.handle.read_bulk(self.read_endpoint, buf, self.timeout)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        self.handle
            .write_bulk(self.write_endpoint, buf, self.timeout)
    }
}

impl io::Read for Handle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.handle.read_bulk(self.read_endpoint, buf, self.timeout) {
            Ok(n) => Ok(n),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }
}

impl io::Write for Handle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self
            .handle
            .write_bulk(self.write_endpoint, buf, self.timeout)
        {
            Ok(n) => Ok(n),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn detect(_timeout: Duration) -> Result<Devices, Error> {
    let list = rusb::Context::new()?.devices()?;

    let mut devices = vec![];
    for device in list.iter() {
        let desc = device.device_descriptor()?;

        if desc.vendor_id() != VENDOR_ID && !PRODUCT_IDS.iter().any(|id| *id == desc.product_id()) {
            continue;
        }

        usb_debug!(device, "found Samsung device: {:?}", device);

        let cd = device.config_descriptor(0)?;

        let mut it = cd.interfaces().flat_map(|iface| iface.descriptors());

        let config = loop {
            match it.next() {
                Some(iface)
                    if iface.class_code() == LIBUSB_CLASS_DATA && iface.num_endpoints() == 2 =>
                {
                    let iface_number = iface.interface_number();
                    let alt_setting = iface.setting_number();
                    let endpoints = iface.endpoint_descriptors().collect::<Vec<_>>();

                    let (read, write) = match endpoints[0].direction() {
                        rusb::Direction::In => (endpoints[0].address(), endpoints[1].address()),
                        rusb::Direction::Out => (endpoints[1].address(), endpoints[0].address()),
                    };

                    break Some((iface_number, alt_setting, read, write));
                }
                Some(_) => continue,
                None => break None,
            }
        };

        if let Some((iface, alt_setting, read, write)) = config {
            usb_debug!(
                device,
                "interface[{}].altsetting[{}]: in={:02X} out={:02X}",
                iface,
                alt_setting,
                read,
                write
            );
            devices.push(Device {
                device,
                iface_number: iface,
                alt_setting,
                read_endpoint: read,
                write_endpoint: write,
            });
        }
    }

    Ok(Devices(devices))
}
