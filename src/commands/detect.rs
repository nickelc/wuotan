use std::time::Duration;

use clap::ArgMatches;

use super::{App, CliResult};
use crate::device;

pub fn cli() -> App {
    App::new("detect").about("list connected Samsung devices")
}

pub fn exec(_args: &ArgMatches<'_>) -> CliResult {
    for device in device::detect(Duration::from_secs(1))? {
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
