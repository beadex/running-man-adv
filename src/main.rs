mod bus;
mod cpu;
mod frontend;
mod gba;
mod loader;

use std::{env, error::Error, ffi::OsString, fmt, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};

use crate::{
    frontend::sdl,
    gba::Gba,
    loader::{load_bios_file, load_rom_file},
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,

        Err(error) => {
            eprintln!("error: {error}");

            let mut source = error.source();

            while let Some(cause) = source {
                eprintln!("caused by: {cause}");
                source = cause.source();
            }

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let config = Config::from_args(env::args_os())?;

    let bios = load_bios_file(&config.bios_path)
        .with_context(|| format!("failed to load BIOS from {}", config.bios_path.display()))?;

    let rom = load_rom_file(&config.rom_path)
        .with_context(|| format!("failed to load ROM from {}", config.rom_path.display()))?;

    println!("GBA emulator starting...");
    println!("BIOS: {}", config.bios_path.display());
    println!("ROM:  {}", config.rom_path.display());
    println!("Title:      {}", rom.header().title());
    println!("Game code:  {}", rom.header().game_code());
    println!("Maker code: {}", rom.header().maker_code());
    println!("Version:    {}", rom.header().software_version());
    println!("ROM size:   {} bytes", rom.bytes().len());

    let gba = Gba::with_images(bios.bytes(), rom.bytes())
        .context("failed to initialize the GBA machine")?;

    sdl::run(gba).context("SDL frontend failed")
}

#[derive(Debug)]
struct Config {
    bios_path: PathBuf,
    rom_path: PathBuf,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self, CliError> {
        let mut args = args.into_iter();

        let executable = args
            .next()
            .unwrap_or_else(|| OsString::from("gba-emulator"));

        let mut bios_path = None;
        let mut rom_path = None;

        while let Some(argument) = args.next() {
            match argument.to_str() {
                Some("--bios") => {
                    let value = args
                        .next()
                        .ok_or(CliError::MissingOptionValue { option: "--bios" })?;

                    bios_path = Some(PathBuf::from(value));
                }

                Some("--rom") => {
                    let value = args
                        .next()
                        .ok_or(CliError::MissingOptionValue { option: "--rom" })?;

                    rom_path = Some(PathBuf::from(value));
                }

                Some("--help" | "-h") => {
                    return Err(CliError::HelpRequested {
                        executable: PathBuf::from(executable),
                    });
                }

                _ => {
                    return Err(CliError::UnknownArgument(argument));
                }
            }
        }

        let bios_path = bios_path.ok_or(CliError::MissingRequiredOption { option: "--bios" })?;

        let rom_path = rom_path.ok_or(CliError::MissingRequiredOption { option: "--rom" })?;

        Ok(Self {
            bios_path,
            rom_path,
        })
    }
}

#[derive(Debug)]
enum CliError {
    MissingRequiredOption { option: &'static str },
    MissingOptionValue { option: &'static str },
    UnknownArgument(OsString),
    HelpRequested { executable: PathBuf },
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredOption { option } => {
                write!(
                    formatter,
                    "missing required option {option}\n\n{}",
                    usage("gba-emulator"),
                )
            }

            Self::MissingOptionValue { option } => {
                write!(
                    formatter,
                    "missing value for option {option}\n\n{}",
                    usage("gba-emulator"),
                )
            }

            Self::UnknownArgument(argument) => {
                write!(
                    formatter,
                    "unknown argument '{}'\n\n{}",
                    argument.to_string_lossy(),
                    usage("gba-emulator"),
                )
            }

            Self::HelpRequested { executable } => {
                write!(formatter, "{}", usage(&executable.to_string_lossy()))
            }
        }
    }
}

impl Error for CliError {}

fn usage(executable: &str) -> String {
    format!(
        "\
Usage:
  {executable} --bios <bios-file> --rom <rom-file>

Options:
  --bios <path>    Path to a legally dumped 16 KiB GBA BIOS
  --rom <path>     Path to a GBA ROM image
  -h, --help       Show this help message
"
    )
}
