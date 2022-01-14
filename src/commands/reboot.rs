use std::time::Duration;

use clap::ArgMatches;

use crate::proto;

use super::{App, AppExt, ArgMatchesExt, CliResult};

pub fn cli() -> App {
    App::new("reboot")
        .about("reboot a connected device")
        .arg_select_device()
}

pub fn exec(args: &ArgMatches) -> CliResult {
    if let Some(device) = args.selected_device()? {
        let mut handle = device.open(Duration::from_secs(3))?;
        handle.claim()?;
        handle.reset()?;

        proto::handshake(&handle)?;
        proto::reboot(&handle)?;

        handle.release()?;

        println!("Rebooting...");
    }

    Ok(())
}
