use std::fs::File;
use std::io::{BufReader, Cursor};
use std::time::Duration;

use clap::ArgMatches;

use super::{App, AppExt, ArgMatchesExt, CliResult};
use crate::pit::{self, Pit};
use crate::proto;

pub fn cli() -> App {
    App::new("flash")
        .about("flash partitions to a connected device")
        .arg_from_usage("<part> -p <NAME> <FILE>..., --partition 'partition name and file'")
        .arg_from_usage("--reboot 'reboot device after upload'")
        .arg_select_device()
}

pub fn exec(args: &ArgMatches<'_>) -> CliResult {
    let values = args.values_of("part").map(chunked).expect("required args");

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

    if let Some(device) = args.selected_device()? {
        let mut handle = device.open(Duration::from_secs(15))?;
        handle.claim()?;
        handle.reset()?;

        proto::handshake(&handle)?;

        if proto::begin_session(&handle)? != 0 {
            proto::setup_file_part_size(&handle, 1048576)?; // 1MB
        }

        let pit = proto::receive_pit(&handle)?;
        let pit = Pit::from_read(&mut Cursor::new(pit))?;

        let part_info = part_info
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
                pit::BinaryType::ApplicationProcessor => proto::FileTarget::ApplicationProcessor {
                    device_type: entry.device_type.as_u32(),
                    identifier: entry.identifier,
                },
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

        if args.is_present("reboot") {
            proto::reboot(&handle)?;
        }

        handle.release()?;
    }
    Ok(())
}

fn open_file_image(name: &str) -> Result<(BufReader<File>, u64), FlashError> {
    let error = |e| FlashError::FileImage(name.to_owned(), e);

    let file = File::open(&name).map_err(error)?;
    let md = file.metadata().map_err(error)?;
    if !md.is_file() {
        return Err(error(io::Error::new(io::ErrorKind::Other, "not a file")));
    }
    let file_size = md.len();

    Ok((BufReader::new(file), file_size))
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
