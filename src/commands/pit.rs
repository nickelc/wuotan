use std::fs::File;
use std::io::{BufReader, Cursor, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::{AppSettings, Arg, ArgMatches};

use super::{App, AppExt, ArgMatchesExt, CliResult, Error};
use crate::device::Handle;
use crate::pit::Pit;
use crate::proto;

pub fn cli() -> App {
    App::new("pit")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            App::new("print")
                .about("print the contents of the PIT from a connected device or a PIT file")
                .arg(
                    Arg::new("file")
                        .long("file")
                        .short('f')
                        .value_name("FILE")
                        .allow_invalid_utf8(true)
                        .help("read local PIT file"),
                )
                .arg_select_device(),
        )
        .subcommand(
            App::new("download")
                .about("save the PIT from a connected device to a specific file")
                .arg(
                    Arg::new("output")
                        .value_name("OUTPUT")
                        .required(true)
                        .allow_invalid_utf8(true)
                        .help("path to the output file"),
                )
                .arg_select_device(),
        )
}

pub fn exec(args: &ArgMatches) -> CliResult {
    match args.subcommand() {
        Some(("download", args)) => download(args),
        Some(("print", args)) => print(args),
        _ => unreachable!(),
    }
}

fn download(args: &ArgMatches) -> CliResult {
    let output = args.value_of_os("output").expect("argument is required");
    let output = PathBuf::from(output);

    if output.exists() {
        return Err("output file already exists".into());
    }

    if let Some(device) = args.selected_device()? {
        let mut handle = device.open(Duration::from_secs(3))?;
        handle.claim()?;
        handle.reset()?;

        let pit = download_pit(&handle)?;

        handle.release()?;

        let mut output = File::create(output)?;
        output.write_all(&pit)?;

        println!("PIT download successful");
    }
    Ok(())
}

fn print(args: &ArgMatches) -> CliResult {
    if args.is_present("file") {
        let input = args.value_of_os("file").unwrap();
        let mut input = BufReader::new(File::open(input)?);
        let pit = Pit::from_read(&mut input)?;
        print_pit(&pit);
    } else if let Some(device) = args.selected_device()? {
        let mut handle = device.open(Duration::from_secs(3))?;
        handle.claim()?;
        handle.reset()?;

        let pit = download_pit(&handle)?;

        handle.release()?;

        let mut buf = Cursor::new(pit);
        let pit = Pit::from_read(&mut buf)?;
        print_pit(&pit);
    }
    Ok(())
}

fn download_pit(handle: &Handle) -> Result<Vec<u8>, Error> {
    proto::handshake(handle)?;
    proto::begin_session(handle)?;

    let data = proto::receive_pit(handle)?;

    proto::end_session(handle)?;
    Ok(data)
}

fn print_pit(pit: &Pit) {
    use crate::pit::{Attributes, UpdateAttributes};

    println!("Entry Count: {}", pit.entries.len());
    println!("Unknown 1: {}", pit.unknown1);
    println!("Unknown 2: {}", pit.unknown2);
    println!("Unknown 3: {}", pit.unknown3);
    println!("Unknown 4: {}", pit.unknown4);
    println!("Unknown 5: {}", pit.unknown5);
    println!("Unknown 6: {}", pit.unknown6);
    println!("Unknown 7: {}", pit.unknown7);
    println!("Unknown 8: {}\n", pit.unknown8);

    for (i, e) in pit.entries.iter().enumerate() {
        println!("--- Entry #{} ---", i);
        println!(
            "Binary Type: {} ({})",
            e.binary_type.as_u32(),
            e.binary_type
        );
        println!(
            "Device Type: {} ({})",
            e.device_type.as_u32(),
            e.device_type
        );
        println!("Identifier: {}", e.identifier);
        let mut attr_s = String::new();
        if e.attributes.contains(Attributes::STL) {
            attr_s.push_str("STL ");
        }
        if e.attributes.contains(Attributes::WRITE) {
            attr_s.push_str("Read/Write");
        } else {
            attr_s.push_str("Read-Only");
        }
        println!("Attributes: {:08b} ({})", e.attributes.bits(), attr_s);
        let mut attr_s = String::new();
        if e.update_attributes.contains(UpdateAttributes::FOTA) {
            attr_s.push_str("FOTA");
        }
        if e.update_attributes.contains(UpdateAttributes::SECURE) {
            if attr_s.is_empty() {
                attr_s.push_str("Secure");
            } else {
                attr_s.push_str(", Secure");
            }
        }
        println!(
            "Update Attributes: {:08b} ({})",
            e.attributes.bits(),
            attr_s
        );
        println!("Partition Block Size/Offset: {}", e.blocksize_or_offset);
        println!("Partition Block Count: {}", e.block_count);
        println!("File Offset (Obsolete): {}", e.file_offset);
        println!("File Size (Obsolete): {}", e.file_size);
        println!("Partition Name: {}", e.partition_name);
        println!("Flash Name: {}", e.flash_filename);
        println!("FOTA Name: {}\n", e.fota_filename);
    }
}
