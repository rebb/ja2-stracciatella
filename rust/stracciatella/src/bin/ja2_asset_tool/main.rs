//! This file contains the code for the ja2-asset-tool executable
//!
//! ja2-asset-tool allows to easily modify Jagged Alliance 2 resources through various subcommands

mod slf;

use clap::{crate_version, App, AppSettings, Arg, ArgMatches, SubCommand};
use image::gif::Encoder as GifEncoder;
use jwalk::WalkDir;
use log::{debug, error, warn};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, Error as IoError, ErrorKind, Read, Result as IoResult, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process;

use stracciatella::config::{find_stracciatella_home, EngineOptions};
use stracciatella::file_formats::stci::Stci;
use stracciatella::graphics::{Animation, AnimationSet, Texture, TextureSet};
use stracciatella::librarydb::LibraryDB;
use stracciatella::logger::{LogLevel, Logger};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FileType {
    Slf,
    Stci,
    Pcx,
    Tga,
    Gap,
    Wav,
    Jsd,
    Unknown,
}

#[derive(Debug, Clone, Default)]
struct Statistics {
    analyzed: Vec<String>,
    file_types: HashMap<FileType, u64>,
}

fn main() {
    let cmd_create = SubCommand::with_name("statistics")
        .about("Prints some statistics about your current data files.")
        .arg(
            Arg::with_name("directory")
                .help("Manually specify a directory to scan")
                .long("directory")
                .takes_value(true),
        );

    let app = App::new("ja2-asset-tool")
        .about("Tool allows managing and converting from and to Jagged Alliance 2 assets.")
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("debug")
                .help("Prints some debug output")
                .long("debug"),
        )
        .subcommand(cmd_create);
    let matches = app.get_matches();

    Logger::init(&Path::new("ja2-asset-tool.log"));
    let level = if matches.is_present("debug") {
        LogLevel::Debug
    } else {
        LogLevel::Warn
    };
    Logger::set_level(level);

    match matches.subcommand() {
        ("statistics", Some(matches)) => subcommand_statistics(matches),
        _ => unreachable!(),
    }
}

fn file_type_from_path(path: &Path) -> FileType {
    let extension = path.extension();

    match extension {
        Some(s) => {
            let s = s.to_string_lossy().to_lowercase();
            match s.as_str() {
                "slf" => FileType::Slf,
                "sti" => FileType::Stci,
                "pcx" => FileType::Pcx,
                "tga" => FileType::Tga,
                "gap" => FileType::Gap,
                "wav" => FileType::Wav,
                "jsd" => FileType::Jsd,
                _ => FileType::Unknown,
            }
        }
        None => FileType::Unknown,
    }
}

fn read_file<R>(
    state: &mut Statistics,
    archive: Option<&Path>,
    file_name: &Path,
    content: &mut R,
) -> IoResult<()>
where
    R: Read + Seek,
{
    let file_id = if let Some(archive) = archive {
        format!("{}#{}", archive.display(), file_name.display())
    } else {
        format!("{}", file_name.display())
    };
    let file_type = file_type_from_path(&file_name);

    debug!("File {} has type {:?}", file_id, file_type);
    match file_type {
        FileType::Stci => {
            state
                .file_types
                .entry(FileType::Stci)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
            let mut buf_reader = BufReader::new(content);
            // Check AnimationSet
            buf_reader.seek(SeekFrom::Start(0))?;
            let animation_set = AnimationSet::read(&mut buf_reader);
            if let Ok(animation) = &animation_set {
                let filename = file_id.replace('/', "_").replace('.', "_");
                warn!("animation set loaded writing {}", filename);
                let frames = animation.clone().into_frames().unwrap();
                for (index, frames) in frames.into_iter().enumerate() {
                    let filename = format!("data/{}_{}.gif", filename, index);
                    let file_out = File::create(filename)?;
                    let mut encoder = GifEncoder::new(file_out);
                    encoder.encode_frames(frames).unwrap();
                }
                return Ok(());
            }
            // Check Animation
            buf_reader.seek(SeekFrom::Start(0))?;
            let animation = Animation::read(&mut buf_reader);
            if let Ok(animation) = &animation {
                let mut filename = file_id.replace('/', "_").replace('.', "_");
                filename.push_str(".gif");
                let filename = format!("data/{}", filename);
                warn!("animation loaded writing {}", filename);
                let frames = animation.clone().into_frames().unwrap();
                let file_out = File::create(filename)?;
                let mut encoder = GifEncoder::new(file_out);
                encoder.encode_frames(frames.into_iter()).unwrap();
                return Ok(());
            }
            // Check TextureSet
            buf_reader.seek(SeekFrom::Start(0))?;
            let texture_set = TextureSet::read(&mut buf_reader);
            if let Ok(texture_set) = &texture_set {
                let filename = file_id.replace('/', "_").replace('.', "_");
                warn!("texture set loaded writing {}", filename);
                let frames = texture_set.clone().into_images().unwrap();
                for (index, img) in frames.into_iter().enumerate() {
                    let filename = format!("data/{}_{}.gif", filename, index);
                    img.save_with_format(filename, image::ImageFormat::Png)
                        .unwrap();
                }
                return Ok(());
            }
            // Check Texture
            buf_reader.seek(SeekFrom::Start(0))?;
            let texture = Texture::read(&mut buf_reader);
            if let Ok(texture) = &texture {
                let mut filename = file_id.replace('/', "_").replace('.', "_");
                filename.push_str(".png");
                let filename = format!("data/{}", filename);
                warn!("texture loaded writing {}", filename);
                let img = texture.clone().into_image().unwrap();
                img.save_with_format(filename, image::ImageFormat::Png)
                    .unwrap();
                return Ok(());
            }
            error!(
                "could not load as any concrete object:\nAnimation: {:?}\nTexure: {:?}",
                animation.err().unwrap(),
                texture.err().unwrap()
            );
        }
        FileType::Slf => {
            state
                .file_types
                .entry(FileType::Slf)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);

            if archive.is_some() {
                return Err(IoError::new(ErrorKind::InvalidData, "nested slf detected"));
            }

            let base_dir = file_name
                .parent()
                .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "slf should have a parent"))?;
            let library_name = Path::new(file_name.file_name().ok_or_else(|| {
                IoError::new(ErrorKind::InvalidData, "slf should have a filename")
            })?);
            let mut library_db = LibraryDB::new();
            library_db.add_library(&base_dir, &library_name)?;

            let files = library_db.list_files();
            for library_file_name in &files {
                let mut file = library_db.open_file(&library_file_name)?;
                let library_file_name = Path::new(library_file_name);
                read_file(state, Some(file_name), library_file_name, &mut file)?;
            }
        }
        FileType::Pcx => {
            state
                .file_types
                .entry(FileType::Pcx)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
        FileType::Tga => {
            state
                .file_types
                .entry(FileType::Tga)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
        FileType::Gap => {
            state
                .file_types
                .entry(FileType::Gap)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
        FileType::Wav => {
            state
                .file_types
                .entry(FileType::Wav)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
        FileType::Jsd => {
            state
                .file_types
                .entry(FileType::Jsd)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
        FileType::Unknown => {
            state
                .file_types
                .entry(FileType::Unknown)
                .and_modify(|f: &mut u64| *f += 1)
                .or_insert(1);
        }
    };

    state.analyzed.push(file_id);
    Ok(())
}

fn subcommand_statistics(matches: &ArgMatches) {
    let directory: PathBuf = if let Some(value) = matches.value_of("directory") {
        value.into()
    } else {
        let stracciatella_home = graceful_unwrap(
            "Error determining stracciatella home",
            find_stracciatella_home(),
        );
        let engine_options =
            EngineOptions::from_home_and_args(&stracciatella_home, &["ja2-asset-tool".to_owned()]);
        let engine_options = graceful_unwrap("Error parsing config", engine_options);
        engine_options.vanilla_game_dir
    };
    let mut state = Statistics::default();

    debug!("Directory to walk: {:?}", directory);
    for entry in WalkDir::new(directory).sort(true) {
        let entry = graceful_unwrap("error reading dir entry", entry);
        let path = entry.path();
        let mut file =
            graceful_unwrap(&format!("error opening file {:?}", path), File::open(&path));

        graceful_unwrap(
            &format!("error reading file {:?}", path),
            read_file(&mut state, None, &path, &mut file),
        );
    }

    // println!("{:?}", state);
}

/// Either unwraps a result or prints an error to stderr and exits with 1.
fn graceful_unwrap<T, E: Debug>(desc: &str, result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{}: {:?}", desc, err);
            process::exit(1);
        }
    }
}

/// Prints an error to stderr and exits with 1.
fn graceful_error(desc: &str) {
    eprintln!("{}", desc);
    process::exit(1);
}
