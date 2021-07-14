use std::fs::File;
use std::io::{BufReader, Cursor};
use std::time::Duration;

use clap::{crate_description, crate_name, crate_version};
use clap::{App, AppSettings};

#[macro_use]
mod macros;
mod device;
mod pit;
mod proto;

use crate::pit::Pit;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let app = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(App::new("detect").about("list connected Samsung devices"))
        .subcommand(
            App::new("print-pit")
                .about("print the contents of the PIT from a connected device or a PIT file")
                .arg_from_usage("[file] -f <FILE>, --file 'read local PIT file'"),
        )
        .subcommand(
            App::new("flash")
                .about("flash partitions to a connected device")
                .arg_from_usage("<part> -p <NAME> <FILE>..., --partition 'partition name and file'")
                .arg_from_usage("--reboot 'reboot device after upload'"),
        );

    match app.get_matches().subcommand() {
        ("detect", Some(_matches)) => {
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
        }
        ("print-pit", Some(matches)) if matches.is_present("file") => {
            let input = matches.value_of_os("file").unwrap();
            let mut input = BufReader::new(File::open(input)?);
            let pit = Pit::from_read(&mut input)?;
            print_pit(&pit);
        }
        ("print-pit", Some(_matches)) => {
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
        ("flash", Some(matches)) => {
            let values = matches
                .values_of("part")
                .map(chunked)
                .expect("required args");

            let fold_values = |(mut acc, total): (Vec<(_, (_, _))>, _), info| match info {
                Ok((name, (file, file_size))) => {
                    acc.push((name, (file, file_size)));
                    Ok((acc, total + file_size))
                }
                Err(e) => Err(e),
            };
            let (part_info, total_file_size) = values
                .map(|(name, file)| open_file_image(file).map(|f| (name, f)))
                .try_fold((Vec::new(), 0), fold_values)?;

            let devices = device::detect(Duration::from_secs(1))?;
            if let Some(device) = devices.iter().next() {
                let mut handle = device.open(Duration::from_secs(15))?;
                handle.claim().ok();
                handle.reset().ok();

                proto::handshake(&handle)?;

                if proto::begin_session(&handle)? != 0 {
                    proto::setup_file_part_size(&handle, 1048576)?; // 1MB
                }

                let pit = proto::receive_pit(&handle)?;
                let pit = Pit::from_read(&mut Cursor::new(pit))?;

                let part_info =
                    part_info
                        .into_iter()
                        .try_fold(Vec::new(), |mut acc, (name, file)| {
                            let entry = pit
                                .entries
                                .iter()
                                .find(|e| e.partition_name.eq_ignore_ascii_case(name.as_bytes()))
                                .ok_or_else(|| FlashError::PartitionNotFound(name.to_owned()))?;

                            acc.push((entry, file));
                            Ok::<_, FlashError>(acc)
                        });
                let part_info = match part_info {
                    Ok(part_info) => part_info,
                    Err(e) => {
                        proto::end_session(&handle)?;
                        return Err(e.into());
                    }
                };

                proto::send_total_size(&handle, total_file_size as u32)?;

                for (entry, (mut file, file_size)) in part_info {
                    println!("Uploading {}", entry.partition_name);

                    let target = match entry.binary_type {
                        pit::BinaryType::ApplicationProcessor => {
                            proto::FileTarget::ApplicationProcessor {
                                device_type: entry.device_type.as_u32(),
                                identifier: entry.identifier,
                            }
                        }
                        pit::BinaryType::CommunicationProcessor => {
                            proto::FileTarget::CommunicationProcessor {
                                device_type: entry.device_type.as_u32(),
                            }
                        }
                        pit::BinaryType::Unknown(_) => todo!(),
                    };

                    proto::file_transfer(&handle, &target, &mut file, file_size)?;

                    println!("{} upload successful", entry.partition_name);
                }

                proto::end_session(&handle)?;

                if matches.is_present("reboot") {
                    proto::reboot(&handle)?;
                }

                handle.release().ok();
            }
        }
        _ => unreachable!(),
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

fn open_file_image(name: &str) -> Result<(File, u64), FlashError> {
    let error = |e| FlashError::FileImage(name.to_owned(), e);

    let file = File::open(&name).map_err(error)?;
    let md = file.metadata().map_err(error)?;
    if !md.is_file() {
        return Err(error(io::Error::new(io::ErrorKind::Other, "not a file")));
    }
    let file_size = md.len();

    Ok((file, file_size))
}

fn chunked<T: Iterator>(mut iter: T) -> impl Iterator<Item = (T::Item, T::Item)> {
    std::iter::from_fn(move || match (iter.next(), iter.next()) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    })
}

use std::fmt;
use std::io;

#[derive(Debug)]
enum FlashError {
    PartitionNotFound(String),
    FileImage(String, io::Error),
}

impl std::error::Error for FlashError {}

impl fmt::Display for FlashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlashError::PartitionNotFound(name) => write!(f, r#"partition not found: "{}""#, name),
            FlashError::FileImage(file, e) if e.kind() == io::ErrorKind::NotFound => {
                write!(f, "file image not found: {}", file)
            }
            FlashError::FileImage(file, _) => write!(f, r#"invalid file image: "{}""#, file),
        }
    }
}
