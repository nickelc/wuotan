use clap::{Arg, ArgMatches};

mod detect;
mod flash;
mod pit;
mod reboot;

use crate::device::{self, Device};
use crate::error::{CliResult, Error};

pub type App = clap::App<'static>;

pub fn cli() -> Vec<App> {
    vec![detect::cli(), pit::cli(), flash::cli(), reboot::cli()]
}

pub fn get(cmd: &str) -> Option<fn(&ArgMatches) -> CliResult> {
    let func = match cmd {
        "detect" => detect::exec,
        "pit" => pit::exec,
        "flash" => flash::exec,
        "reboot" => reboot::exec,
        _ => return None,
    };
    Some(func)
}

pub trait AppExt {
    fn arg_usb_log_level(self) -> App;

    fn arg_select_device(self) -> App;
}

impl AppExt for App {
    fn arg_usb_log_level(self) -> App {
        self.arg(
            Arg::new("usb-log-level")
                .long("usb-log-level")
                .help("set the libusb log level")
                .global(true)
                .takes_value(true)
                .value_name("LEVEL")
                .possible_values(&["error", "warn", "info", "debug"]),
        )
    }

    fn arg_select_device(self) -> App {
        self.arg(
            Arg::new("device")
                .long("device")
                .short('d')
                .value_name("DEVICE")
                .help(r#"select a device via bus number and its address (ex: "003:068", "3:68")"#)
                .validator(|s| match s.split_once(':') {
                    Some((bus_number, address)) => {
                        bus_number.parse::<u8>().map_err(|_| "invalid bus number")?;
                        address
                            .parse::<u8>()
                            .map_err(|_| "invalid device address")?;
                        Ok(())
                    }
                    _ => Err(r#"invalid device selector. expected: "XXX:XXX""#),
                }),
        )
    }
}

pub trait ArgMatchesExt {
    fn usb_log_level(&self) -> Option<rusb::LogLevel>;

    fn selected_device(&self) -> Result<Option<Device>, Error>;
}

impl ArgMatchesExt for ArgMatches {
    fn usb_log_level(&self) -> Option<rusb::LogLevel> {
        let level = match self.value_of("usb-log-level") {
            Some("error") => rusb::LogLevel::Error,
            Some("warn") => rusb::LogLevel::Warning,
            Some("info") => rusb::LogLevel::Info,
            Some("debug") => rusb::LogLevel::Debug,
            _ => return None,
        };
        Some(level)
    }

    fn selected_device(&self) -> Result<Option<Device>, Error> {
        let level = self.usb_log_level();
        let mut it = device::detect(level)?.into_iter();

        let device = match self.value_of("device").and_then(|s| s.split_once(':')) {
            Some((bus_number, address)) => {
                let bus_number = bus_number.parse::<u8>()?;
                let address = address.parse::<u8>()?;
                it.find(|d| d.bus_number() == bus_number && d.address() == address)
            }
            None => it.next(),
        };

        Ok(device)
    }
}

pub fn opt(name: &'static str, help: &'static str) -> Arg<'static> {
    Arg::new(name).long(name).help(help)
}

/// Allow invalid utf8 for path/file options
pub fn path_opt(name: &'static str, help: &'static str) -> Arg<'static> {
    opt(name, help).allow_invalid_utf8(true)
}
