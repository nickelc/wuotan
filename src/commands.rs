use clap::{Arg, ArgMatches};

mod detect;
mod flash;
mod pit;

use crate::error::{CliResult, Error};

pub type App = clap::App<'static, 'static>;

pub fn cli() -> Vec<App> {
    vec![detect::cli(), pit::cli(), flash::cli()]
}

pub fn get(cmd: &str) -> Option<fn(&ArgMatches<'_>) -> CliResult> {
    let func = match cmd {
        "detect" => detect::exec,
        "pit" => pit::exec,
        "flash" => flash::exec,
        _ => return None,
    };
    Some(func)
}

pub trait AppExt {
    fn arg_usb_log_level(self) -> App;
}

impl AppExt for App {
    fn arg_usb_log_level(self) -> App {
        self.arg(
            Arg::with_name("usb-log-level")
                .long("usb-log-level")
                .help("set the libusb log level")
                .global(true)
                .takes_value(true)
                .value_name("LEVEL")
                .possible_values(&["error", "warn", "info", "debug"]),
        )
    }
}

pub trait ArgMatchesExt {
    fn usb_log_level(&self) -> Option<rusb::LogLevel>;
}

impl ArgMatchesExt for ArgMatches<'_> {
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
}
