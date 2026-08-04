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

    let mut gba = Gba::with_images(bios.bytes(), rom.bytes())
        .context("failed to initialize the GBA machine")?;

    gba.cpu_mut().set_strict_faults(config.strict_cpu);

    if let Some(cycle_budget) = config.headless_cycles {
        run_headless(gba, cycle_budget, config.watch_address).context("headless run failed")
    } else {
        sdl::run(gba).context("SDL frontend failed")
    }
}

fn run_headless(mut gba: Gba, cycle_budget: u64, watch_address: Option<u32>) -> Result<()> {
    let starting_cycles = gba.elapsed_cycles();
    let mut watch_stats = watch_address.map(|address| WatchStats::new(gba.bus().read32(address)));

    while gba.elapsed_cycles().wrapping_sub(starting_cycles) < cycle_budget {
        let cycles = gba.step();

        if let (Some(address), Some(stats)) = (watch_address, watch_stats.as_mut()) {
            stats.observe(gba.bus().read32(address));
        }

        if cycles == 0 {
            break;
        }
    }

    let consumed = gba.elapsed_cycles().wrapping_sub(starting_cycles);
    let registers = gba.registers();
    let cpsr = registers.cpsr();
    let bus = gba.bus();

    println!("Headless run complete:");
    println!("  requested cycles: {cycle_budget}");
    println!("  consumed cycles:  {consumed}");
    println!("  PC:               0x{:08X}", registers.pc());
    println!("  CPSR:             0x{:08X}", cpsr.raw());
    println!("  CPU state:        {:?}", gba.state());
    println!("  CPU mode:         {:?}", registers.mode());
    println!("  IRQ disabled:     {}", cpsr.irq_disabled());
    println!("  halted:           {}", gba.cpu().is_halted());
    println!(
        "  IE:               0x{:04X}",
        bus.read16(crate::bus::Bus::REG_IE)
    );
    println!(
        "  IF:               0x{:04X}",
        bus.read16(crate::bus::Bus::REG_IF)
    );
    println!(
        "  IME:              0x{:04X}",
        bus.read16(crate::bus::Bus::REG_IME)
    );
    println!(
        "  DISPSTAT:         0x{:04X}",
        bus.read16(crate::bus::Bus::REG_DISPSTAT)
    );
    println!(
        "  DISPCNT:          0x{:04X}",
        bus.read16(crate::bus::Bus::REG_DISPCNT)
    );
    println!(
        "  VCOUNT:           {}",
        bus.read16(crate::bus::Bus::REG_VCOUNT)
    );
    println!("  frame:            {}", gba.frame_number());
    println!(
        "  framebuffer hash: 0x{:016X}",
        framebuffer_hash(gba.framebuffer())
    );
    println!("  IRQ handler:      0x{:08X}", bus.read32(0x0300_7FFC));

    if let (Some(address), Some(stats)) = (watch_address, watch_stats) {
        println!("  [0x{address:08X}]:     0x{:08X}", stats.last);
        println!("  watch changes:    {}", stats.changes);

        for (bit, count) in stats.rising_edges.into_iter().enumerate() {
            if count != 0 {
                println!("  watch bit {bit:>2} rises: {count}");
            }
        }
    }

    if let Some(fault) = gba.cpu().fault() {
        println!("  CPU fault:         {fault:?}");

        anyhow::bail!(
            "CPU fault at 0x{:08X}: {}",
            fault.instruction_address,
            fault.detail
        );
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchStats {
    last: u32,
    changes: u64,
    rising_edges: [u64; u32::BITS as usize],
}

impl WatchStats {
    fn new(initial: u32) -> Self {
        Self {
            last: initial,
            changes: 0,
            rising_edges: [0; u32::BITS as usize],
        }
    }

    fn observe(&mut self, value: u32) {
        if value == self.last {
            return;
        }

        self.changes = self.changes.wrapping_add(1);

        let rising = value & !self.last;

        for bit in 0..u32::BITS as usize {
            if rising & (1u32 << bit) != 0 {
                self.rising_edges[bit] = self.rising_edges[bit].wrapping_add(1);
            }
        }

        self.last = value;
    }
}

fn framebuffer_hash(framebuffer: &[u32]) -> u64 {
    /*
     * Stable FNV-1a hash for comparing headless frames across runs without
     * writing image artifacts to disk.
     */
    let mut hash = 0xCBF2_9CE4_8422_2325u64;

    for pixel in framebuffer {
        for byte in pixel.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    hash
}

#[derive(Debug)]
struct Config {
    bios_path: PathBuf,
    rom_path: PathBuf,
    headless_cycles: Option<u64>,
    watch_address: Option<u32>,
    strict_cpu: bool,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self, CliError> {
        let mut args = args.into_iter();

        let executable = args
            .next()
            .unwrap_or_else(|| OsString::from("gba-emulator"));

        let mut bios_path = None;
        let mut rom_path = None;
        let mut headless_cycles = None;
        let mut watch_address = None;
        let mut strict_cpu = false;

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

                Some("--headless-cycles") => {
                    let value = args.next().ok_or(CliError::MissingOptionValue {
                        option: "--headless-cycles",
                    })?;

                    headless_cycles = Some(parse_u64_option("--headless-cycles", value)?);
                }

                Some("--watch-address") => {
                    let value = args.next().ok_or(CliError::MissingOptionValue {
                        option: "--watch-address",
                    })?;

                    let parsed = parse_u64_option("--watch-address", value)?;

                    watch_address =
                        Some(
                            u32::try_from(parsed).map_err(|_| CliError::IntegerOutOfRange {
                                option: "--watch-address",
                                value: parsed,
                            })?,
                        );
                }

                Some("--strict-cpu") => {
                    strict_cpu = true;
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

        if strict_cpu && headless_cycles.is_none() {
            return Err(CliError::StrictCpuRequiresHeadless);
        }

        Ok(Self {
            bios_path,
            rom_path,
            headless_cycles,
            watch_address,
            strict_cpu,
        })
    }
}

fn parse_u64_option(option: &'static str, value: OsString) -> Result<u64, CliError> {
    let text = value.to_str().ok_or_else(|| CliError::InvalidInteger {
        option,
        value: value.clone(),
    })?;

    let parsed =
        if let Some(hexadecimal) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            u64::from_str_radix(hexadecimal, 16)
        } else {
            text.parse()
        };

    parsed.map_err(|_| CliError::InvalidInteger { option, value })
}

#[derive(Debug)]
enum CliError {
    MissingRequiredOption {
        option: &'static str,
    },
    MissingOptionValue {
        option: &'static str,
    },
    UnknownArgument(OsString),
    HelpRequested {
        executable: PathBuf,
    },
    InvalidInteger {
        option: &'static str,
        value: OsString,
    },
    IntegerOutOfRange {
        option: &'static str,
        value: u64,
    },
    StrictCpuRequiresHeadless,
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

            Self::InvalidInteger { option, value } => {
                write!(formatter, "invalid integer for {option}: {value:?}")
            }

            Self::IntegerOutOfRange { option, value } => {
                write!(formatter, "value for {option} is out of range: {value}")
            }

            Self::StrictCpuRequiresHeadless => {
                write!(formatter, "--strict-cpu requires --headless-cycles")
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
  --headless-cycles <count>
                    Run without SDL for a fixed CPU-cycle budget
  --watch-address <address>
                    Print a 32-bit memory value after a headless run
  --strict-cpu      Stop a headless run at the first CPU decode/execution fault
  -h, --help       Show this help message
"
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{CliError, Config, WatchStats, framebuffer_hash};

    #[test]
    fn framebuffer_hash_is_stable_and_pixel_sensitive() {
        assert_eq!(framebuffer_hash(&[1, 2, 3]), framebuffer_hash(&[1, 2, 3]));
        assert_ne!(framebuffer_hash(&[1, 2, 3]), framebuffer_hash(&[1, 2, 4]));
    }

    #[test]
    fn watch_stats_counts_changes_and_per_bit_rising_edges() {
        let mut stats = WatchStats::new(0b0010);

        stats.observe(0b0010);
        stats.observe(0b0111);
        stats.observe(0b0100);
        stats.observe(0b0110);

        assert_eq!(stats.last, 0b0110);
        assert_eq!(stats.changes, 3);
        assert_eq!(stats.rising_edges[0], 1);
        assert_eq!(stats.rising_edges[1], 1);
        assert_eq!(stats.rising_edges[2], 1);
    }

    #[test]
    fn parses_headless_diagnostic_options() {
        let config = Config::from_args(
            [
                "gba-emulator",
                "--bios",
                "bios.bin",
                "--rom",
                "game.gba",
                "--headless-cycles",
                "1000000",
                "--watch-address",
                "0x030022DC",
                "--strict-cpu",
            ]
            .map(OsString::from),
        )
        .unwrap();

        assert_eq!(config.headless_cycles, Some(1_000_000));
        assert_eq!(config.watch_address, Some(0x0300_22DC));
        assert!(config.strict_cpu);
    }

    #[test]
    fn strict_cpu_requires_headless_mode() {
        let error = Config::from_args(
            [
                "gba-emulator",
                "--bios",
                "bios.bin",
                "--rom",
                "game.gba",
                "--strict-cpu",
            ]
            .map(OsString::from),
        )
        .unwrap_err();

        assert!(matches!(error, CliError::StrictCpuRequiresHeadless));
    }
}
