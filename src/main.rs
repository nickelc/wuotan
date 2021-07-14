use clap::{crate_description, crate_name, crate_version};
use clap::{App, AppSettings};

#[macro_use]
mod macros;
mod commands;
mod device;
mod error;
mod pit;
mod proto;

use error::CliResult;

fn main() -> CliResult {
    tracing_subscriber::fmt::init();

    let app = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommands(commands::cli());

    match app.get_matches().subcommand() {
        (cmd, Some(args)) => {
            if let Some(cmd) = commands::get(cmd) {
                cmd(args)?;
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
