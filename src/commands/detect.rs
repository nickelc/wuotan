use clap::ArgMatches;

use super::{App, ArgMatchesExt, CliResult};
use crate::device;

pub fn cli() -> App {
    App::new("detect").about("list connected Samsung devices")
}

pub fn exec(args: &ArgMatches) -> CliResult {
    let log_level = args.usb_log_level();
    for device in device::detect(log_level)? {
        let (vendor_id, product_id) = device.id()?;
        println!(
            "Bus {:03} Device {:03}: ID {:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            vendor_id,
            product_id
        );
    }

    Ok(())
}
