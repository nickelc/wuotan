use std::fs::File;
use std::io::{BufReader, Cursor};
use std::time::Duration;

use clap::ArgMatches;

use super::{App, CliResult};
use crate::device;
use crate::pit::Pit;
use crate::proto;

pub fn cli() -> App {
    App::new("print-pit")
        .about("print the contents of the PIT from a connected device or a PIT file")
        .arg_from_usage("[file] -f <FILE>, --file 'read local PIT file'")
}

pub fn exec(args: &ArgMatches<'_>) -> CliResult {
    if args.is_present("file") {
        let input = args.value_of_os("file").unwrap();
        let mut input = BufReader::new(File::open(input)?);
        let pit = Pit::from_read(&mut input)?;
        print_pit(&pit);
    } else {
        let devices = device::detect(Duration::from_secs(1))?;
        if let Some(device) = devices.iter().next() {
            let mut handle = device.open(Duration::from_secs(3))?;
            handle.claim().ok();
            handle.reset().ok();

            proto::handshake(&handle)?;

            proto::begin_session(&handle)?;

            let pit = proto::receive_pit(&handle)?;
            let mut buf = Cursor::new(pit);
            let pit = Pit::from_read(&mut buf)?;
            print_pit(&pit);

            proto::end_session(&handle)?;

            handle.release().ok();
        }
    }
    Ok(())
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
