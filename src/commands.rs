use clap::ArgMatches;

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
