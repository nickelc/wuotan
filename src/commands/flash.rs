use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use clap::{Arg, ArgGroup, ArgMatches};

use super::{App, AppExt, ArgMatchesExt, CliResult, Error};
use crate::pit::{BinaryType, Entry, Pit};
use crate::proto::{self, FileTarget};

pub fn cli() -> App {
    App::new("flash")
        .about("flash partitions to a connected device")
        .arg(
            Arg::with_name("part")
                .help("partition name and file image")
                .long("partition")
                .short("p")
                .value_names(&["NAME", "FILE"])
                .multiple(true),
        )
        .arg(
            Arg::with_name("tar")
                .help("tar file containing the file images to be flashed")
                .long("tar")
                .short("t")
                .value_name("FILE")
                .multiple(true),
        )
        .group(
            ArgGroup::with_name("files")
                .multiple(true)
                .required(true)
                .args(&["tar", "part"]),
        )
        .arg_from_usage("--reboot 'reboot device after upload'")
        .arg_select_device()
}

pub fn exec(args: &ArgMatches<'_>) -> CliResult {
    let files = get_arguments(args)?;

    if let Some(device) = args.selected_device()? {
        let mut handle = device.open(Duration::from_secs(3))?;
        handle.claim()?;
        handle.reset()?;

        proto::handshake(&handle)?;

        if proto::begin_session(&handle)? != 0 {
            proto::setup_file_part_size(&handle, 1048576)?; // 1MB
        }

        let pit = proto::receive_pit(&handle)?;
        let pit = Pit::from_read(&mut Cursor::new(pit))?;

        let (total_file_size, mapped_args) = map_arguments_with_pit(&files, &pit)?;

        proto::send_total_size(&handle, total_file_size as u32)?;

        let target_for_entry = |entry: &Entry| match entry.binary_type {
            BinaryType::ApplicationProcessor => FileTarget::ApplicationProcessor {
                device_type: entry.device_type.as_u32(),
                identifier: entry.identifier,
            },
            BinaryType::CommunicationProcessor => FileTarget::CommunicationProcessor {
                device_type: entry.device_type.as_u32(),
            },
            BinaryType::Unknown(_) => todo!(),
        };

        for entry in mapped_args {
            match entry {
                MappedEntry::Partition { file, entry } => {
                    println!("Uploading {}", entry.partition_name);

                    let target = target_for_entry(entry);
                    let file_size = file.metadata()?.len();
                    let mut file = BufReader::new(File::open(file)?);

                    proto::file_transfer(&handle, &target, &mut file, file_size)?;
                }
                MappedEntry::Tar { file, entries } => {
                    let tar_name = file
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();

                    let mut file = BufReader::new(File::open(file)?);
                    for (entry, (pos, file_size)) in entries {
                        println!("Uploading {}/{}", tar_name, entry.flash_filename);

                        let target = target_for_entry(entry);
                        {
                            file.seek(SeekFrom::Start(pos))?;
                            let mut reader = file.by_ref().take(file_size);
                            proto::file_transfer(&handle, &target, &mut reader, file_size)?;
                        }
                    }
                }
            }
        }

        proto::end_session(&handle)?;

        if args.is_present("reboot") {
            proto::reboot(&handle)?;
            println!("Rebooting...");
        }

        handle.release()?;
    }
    Ok(())
}

enum FileArgument<'a> {
    File { name: &'a str, file: &'a Path },
    Tar { file: &'a Path },
}

impl<'a> FileArgument<'a> {
    fn file(&self) -> &Path {
        match self {
            FileArgument::File { file, .. } => file,
            FileArgument::Tar { file } => file,
        }
    }

    fn is_file(&self) -> bool {
        let file = match self {
            FileArgument::File { file, .. } => file,
            FileArgument::Tar { file } => file,
        };
        file.is_file()
    }
}

fn get_arguments<'a>(args: &'a ArgMatches<'_>) -> Result<Vec<FileArgument<'a>>, Error> {
    fn chunked<T: Iterator>(mut iter: T) -> impl Iterator<Item = (T::Item, T::Item)> {
        std::iter::from_fn(move || iter.next().zip(iter.next()))
    }

    let mut files = vec![];

    let partition_args = args.indices_of("part").zip(args.values_of("part"));
    if let Some((indices, values)) = partition_args {
        let indices = chunked(indices).map(|(i, _)| i);
        let values = chunked(values).map(|(name, file)| FileArgument::File {
            name,
            file: Path::new(file),
        });
        files.extend(indices.zip(values));
    }

    let tar_args = args.indices_of("tar").zip(args.values_of("tar"));
    if let Some((indices, values)) = tar_args {
        let values = values.map(|file| FileArgument::Tar {
            file: Path::new(file),
        });
        files.extend(indices.zip(values));
    }

    files.sort_unstable_by_key(|(idx, _)| *idx);

    for (_, fs) in &files {
        if !fs.is_file() {
            let err = FlashError::InvalidFile(fs.file().display().to_string());
            return Err(err.into());
        }
    }

    Ok(files.into_iter().map(|(_, fs)| fs).collect())
}

enum MappedEntry<'a> {
    Partition {
        file: &'a Path,
        entry: &'a Entry,
    },
    Tar {
        file: &'a Path,
        entries: Vec<(&'a Entry, (u64, u64))>,
    },
}

fn map_arguments_with_pit<'a>(
    files: &'a [FileArgument],
    pit: &'a Pit,
) -> Result<(u64, Vec<MappedEntry<'a>>), Error> {
    let mut total_file_size = 0;
    let mut mapped = vec![];
    for source in files {
        match source {
            FileArgument::File { name, file } => {
                let entry = pit
                    .entries
                    .iter()
                    .find(|e| e.partition_name.eq_ignore_ascii_case(name.as_bytes()))
                    .ok_or_else(|| FlashError::PartitionNotFound(name.to_string()))?;

                total_file_size += file.metadata()?.len();
                mapped.push(MappedEntry::Partition { file, entry });
            }
            FileArgument::Tar { file } => {
                let mut entries = vec![];

                let mut tar = tar::Archive::new(BufReader::new(File::open(file)?));
                for entry in tar.entries()? {
                    let entry = entry?;
                    let path = entry.path()?;
                    let pit_entry = pit
                        .entries
                        .iter()
                        .find(|e| e.flash_filename.eq_ignore_ascii_case(&*entry.path_bytes()))
                        .ok_or_else(|| FlashError::FlashNameNotFound(path.display().to_string()))?;

                    total_file_size += entry.size();
                    entries.push((pit_entry, (entry.raw_file_position(), entry.size())));
                }
                mapped.push(MappedEntry::Tar { file, entries });
            }
        }
    }
    Ok((total_file_size, mapped))
}

use std::fmt;

#[derive(Debug)]
enum FlashError {
    InvalidFile(String),
    PartitionNotFound(String),
    FlashNameNotFound(String),
}

impl std::error::Error for FlashError {}

impl fmt::Display for FlashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlashError::InvalidFile(name) => write!(f, r#"invalid file: "{}""#, name),
            FlashError::PartitionNotFound(name) => write!(f, r#"partition not found: "{}""#, name),
            FlashError::FlashNameNotFound(name) => write!(f, r#"flash name not found: "{}""#, name),
        }
    }
}
