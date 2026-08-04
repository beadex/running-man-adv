mod bus;
mod cpu;
mod frontend;
mod gba;
mod loader;
mod save_file;

use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::{
    bus::Key,
    frontend::sdl,
    gba::Gba,
    loader::{load_bios_file, load_rom_file},
    save_file::SaveFile,
};

const GBA_CLOCK_HZ: u64 = 16_777_216;
const SAVE_FLUSH_INTERVAL_CYCLES: u64 = 5 * GBA_CLOCK_HZ;

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

    let save_file = config
        .save_path
        .as_ref()
        .map(|path| SaveFile::new(path.clone()))
        .unwrap_or_else(|| SaveFile::for_rom(&config.rom_path));

    if gba.cartridge_save_type() == crate::bus::CartridgeSaveType::None {
        println!("Save: none ({})", gba.cartridge_save_type().name());
    } else {
        println!(
            "Save: {} ({} bytes, {})",
            save_file.path().display(),
            gba.cartridge_save_type().size(),
            gba.cartridge_save_type().name()
        );
    }

    if gba.cartridge_has_rtc() {
        println!("RTC:  S3511 (host local clock)");
    }

    if save_file.load(&mut gba)? {
        println!("Loaded save file: {}", save_file.path().display());
    }

    gba.cpu_mut().set_strict_faults(config.strict_cpu);

    let run_result = if let Some(cycle_budget) = config.headless_cycles {
        run_headless(
            &mut gba,
            cycle_budget,
            config.watch_address,
            config.framebuffer_output.as_deref(),
            &config.key_presses,
            &save_file,
        )
        .context("headless run failed")
    } else {
        sdl::run(&mut gba, &save_file).context("SDL frontend failed")
    };

    let flush_result = save_file.flush_if_dirty(&mut gba);

    if matches!(flush_result, Ok(true)) {
        println!("Wrote save file: {}", save_file.path().display());
    }

    run_result?;
    flush_result?;

    Ok(())
}

fn run_headless(
    gba: &mut Gba,
    cycle_budget: u64,
    watch_address: Option<u32>,
    framebuffer_output: Option<&Path>,
    key_presses: &[KeyPress],
    save_file: &SaveFile,
) -> Result<()> {
    let starting_cycles = gba.elapsed_cycles();
    let mut watch_stats = watch_address.map(|address| WatchStats::new(gba.bus().read32(address)));
    let mut pressed_keys = u16::MAX;
    let mut next_save_flush = SAVE_FLUSH_INTERVAL_CYCLES;
    let benchmark_started = Instant::now();

    while gba.elapsed_cycles().wrapping_sub(starting_cycles) < cycle_budget {
        let elapsed = gba.elapsed_cycles().wrapping_sub(starting_cycles);
        let scheduled_keys = scheduled_key_mask(key_presses, elapsed);

        if scheduled_keys != pressed_keys {
            gba.set_pressed_keys(scheduled_keys);
            pressed_keys = scheduled_keys;
        }

        let cycles = gba.step();

        if let (Some(address), Some(stats)) = (watch_address, watch_stats.as_mut()) {
            stats.observe(gba.bus().read32(address));
        }

        if cycles == 0 {
            break;
        }

        if elapsed >= next_save_flush {
            save_file.flush_if_dirty(gba)?;

            while next_save_flush <= elapsed {
                next_save_flush = next_save_flush.saturating_add(SAVE_FLUSH_INTERVAL_CYCLES);
            }
        }
    }

    let consumed = gba.elapsed_cycles().wrapping_sub(starting_cycles);
    let benchmark_elapsed = benchmark_started.elapsed();
    let (emulated_mhz, realtime_percent) = performance_metrics(consumed, benchmark_elapsed);
    let registers = gba.registers();
    let cpsr = registers.cpsr();
    let bus = gba.bus();

    println!("Headless run complete:");
    println!("  requested cycles: {cycle_budget}");
    println!("  consumed cycles:  {consumed}");
    println!(
        "  host time:        {:.3} s",
        benchmark_elapsed.as_secs_f64()
    );
    println!("  emulation rate:   {emulated_mhz:.3} MHz ({realtime_percent:.1}% realtime)");
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
        "  WIN0H/WIN0V:      0x{:04X}/0x{:04X}",
        bus.read16(crate::bus::Bus::REG_WIN0H),
        bus.read16(crate::bus::Bus::REG_WIN0V)
    );
    println!(
        "  WIN1H/WIN1V:      0x{:04X}/0x{:04X}",
        bus.read16(crate::bus::Bus::REG_WIN1H),
        bus.read16(crate::bus::Bus::REG_WIN1V)
    );
    println!(
        "  WININ/WINOUT:     0x{:04X}/0x{:04X}",
        bus.read16(crate::bus::Bus::REG_WININ),
        bus.read16(crate::bus::Bus::REG_WINOUT)
    );
    println!(
        "  VCOUNT:           {}",
        bus.read16(crate::bus::Bus::REG_VCOUNT)
    );
    println!("  frame:            {}", gba.frame_number());
    #[cfg(feature = "perf-stats")]
    {
        let (render_time, rendered_scanlines) = gba.video_render_profile();
        println!("  PPU render time:  {:.3} s", render_time.as_secs_f64());
        println!("  rendered lines:   {rendered_scanlines}");
        println!(
            "  PPU render share: {:.1}%",
            render_time.as_secs_f64() / benchmark_elapsed.as_secs_f64() * 100.0
        );
        let (cpu_steps, cpu_cycles, halt_steps, dma_steps) = gba.scheduler_profile();
        println!("  CPU steps:        {cpu_steps}");
        println!("  CPU cycles:       {cpu_cycles}");
        println!(
            "  cycles/CPU step:  {:.3}",
            cpu_cycles as f64 / cpu_steps.max(1) as f64
        );
        println!("  HALT quanta:      {halt_steps}");
        println!("  DMA runs:         {dma_steps}");
    }
    println!(
        "  framebuffer hash: 0x{:016X}",
        framebuffer_hash(gba.framebuffer())
    );
    println!("  IRQ handler:      0x{:08X}", bus.read32(0x0300_7FFC));

    if let Some(path) = framebuffer_output {
        let mut file = File::create(path)
            .with_context(|| format!("failed to create framebuffer output {}", path.display()))?;

        write_framebuffer_ppm(&mut file, gba.framebuffer())
            .with_context(|| format!("failed to write framebuffer output {}", path.display()))?;

        println!("  framebuffer file: {}", path.display());
    }

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

fn performance_metrics(cycles: u64, elapsed: Duration) -> (f64, f64) {
    let seconds = elapsed.as_secs_f64();

    if seconds == 0.0 {
        return (0.0, 0.0);
    }

    let emulated_hz = cycles as f64 / seconds;
    (
        emulated_hz / 1_000_000.0,
        emulated_hz / GBA_CLOCK_HZ as f64 * 100.0,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyPress {
    key: Key,
    start_cycle: u64,
    duration_cycles: u64,
}

impl KeyPress {
    const fn active_at(self, cycle: u64) -> bool {
        cycle >= self.start_cycle && cycle - self.start_cycle < self.duration_cycles
    }
}

fn scheduled_key_mask(key_presses: &[KeyPress], cycle: u64) -> u16 {
    key_presses.iter().fold(0, |mask, press| {
        if press.active_at(cycle) {
            mask | press.key.mask()
        } else {
            mask
        }
    })
}

fn write_framebuffer_ppm(mut output: impl Write, framebuffer: &[u32]) -> std::io::Result<()> {
    if framebuffer.len() != Gba::SCREEN_WIDTH * Gba::SCREEN_HEIGHT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "framebuffer dimensions do not match the GBA display",
        ));
    }

    write!(
        output,
        "P6\n{} {}\n255\n",
        Gba::SCREEN_WIDTH,
        Gba::SCREEN_HEIGHT
    )?;

    for pixel in framebuffer {
        output.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])?;
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
    framebuffer_output: Option<PathBuf>,
    save_path: Option<PathBuf>,
    key_presses: Vec<KeyPress>,
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
        let mut framebuffer_output = None;
        let mut save_path = None;
        let mut key_presses = Vec::new();
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

                Some("--framebuffer-output") => {
                    let value = args.next().ok_or(CliError::MissingOptionValue {
                        option: "--framebuffer-output",
                    })?;

                    framebuffer_output = Some(PathBuf::from(value));
                }

                Some("--save") => {
                    let value = args
                        .next()
                        .ok_or(CliError::MissingOptionValue { option: "--save" })?;

                    save_path = Some(PathBuf::from(value));
                }

                Some("--press-key") => {
                    let value = args.next().ok_or(CliError::MissingOptionValue {
                        option: "--press-key",
                    })?;

                    key_presses.push(parse_key_press(value)?);
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

        if headless_cycles.is_none() {
            if strict_cpu {
                return Err(CliError::HeadlessOptionRequiresHeadless {
                    option: "--strict-cpu",
                });
            }

            if framebuffer_output.is_some() {
                return Err(CliError::HeadlessOptionRequiresHeadless {
                    option: "--framebuffer-output",
                });
            }

            if !key_presses.is_empty() {
                return Err(CliError::HeadlessOptionRequiresHeadless {
                    option: "--press-key",
                });
            }
        }

        Ok(Self {
            bios_path,
            rom_path,
            headless_cycles,
            watch_address,
            framebuffer_output,
            save_path,
            key_presses,
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

fn parse_key_press(value: OsString) -> Result<KeyPress, CliError> {
    let text = value.to_str().ok_or_else(|| CliError::InvalidKeyPress {
        value: value.clone(),
        detail: "value is not valid Unicode",
    })?;

    let parts: Vec<_> = text.split(':').collect();

    if parts.len() != 3 {
        return Err(CliError::InvalidKeyPress {
            value: value.clone(),
            detail: "expected KEY:START-CYCLE:DURATION-CYCLES",
        });
    }

    let key = match parts[0].to_ascii_uppercase().as_str() {
        "A" => Key::A,
        "B" => Key::B,
        "SELECT" => Key::Select,
        "START" => Key::Start,
        "RIGHT" => Key::Right,
        "LEFT" => Key::Left,
        "UP" => Key::Up,
        "DOWN" => Key::Down,
        "R" => Key::R,
        "L" => Key::L,
        _ => {
            return Err(CliError::InvalidKeyPress {
                value: value.clone(),
                detail: "unknown key name",
            });
        }
    };

    let start_cycle =
        parse_key_press_integer(parts[1]).ok_or_else(|| CliError::InvalidKeyPress {
            value: value.clone(),
            detail: "start cycle is not a valid integer",
        })?;

    let duration_cycles =
        parse_key_press_integer(parts[2]).ok_or_else(|| CliError::InvalidKeyPress {
            value: value.clone(),
            detail: "duration is not a valid integer",
        })?;

    if duration_cycles == 0 {
        return Err(CliError::InvalidKeyPress {
            value,
            detail: "duration must be greater than zero",
        });
    }

    Ok(KeyPress {
        key,
        start_cycle,
        duration_cycles,
    })
}

fn parse_key_press_integer(text: &str) -> Option<u64> {
    if let Some(hexadecimal) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hexadecimal, 16).ok()
    } else {
        text.parse().ok()
    }
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
    HeadlessOptionRequiresHeadless {
        option: &'static str,
    },
    InvalidKeyPress {
        value: OsString,
        detail: &'static str,
    },
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

            Self::HeadlessOptionRequiresHeadless { option } => {
                write!(formatter, "{option} requires --headless-cycles")
            }

            Self::InvalidKeyPress { value, detail } => {
                write!(formatter, "invalid --press-key value {value:?}: {detail}")
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
  --save <path>    Save file path (default: ROM path with .sav extension)
  --headless-cycles <count>
                    Run without SDL for a fixed CPU-cycle budget
  --watch-address <address>
                    Print a 32-bit memory value after a headless run
  --framebuffer-output <path>
                    Write the final headless framebuffer as a binary PPM image
  --press-key <key>:<start>:<duration>
                    Hold a GBA key during a headless cycle interval; repeatable
  --strict-cpu      Stop a headless run at the first CPU decode/execution fault
  -h, --help       Show this help message
"
    )
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::{
        CliError, Config, KeyPress, WatchStats, framebuffer_hash, parse_key_press,
        performance_metrics, scheduled_key_mask, write_framebuffer_ppm,
    };

    use crate::bus::Key;

    #[test]
    fn framebuffer_hash_is_stable_and_pixel_sensitive() {
        assert_eq!(framebuffer_hash(&[1, 2, 3]), framebuffer_hash(&[1, 2, 3]));
        assert_ne!(framebuffer_hash(&[1, 2, 3]), framebuffer_hash(&[1, 2, 4]));
    }

    #[test]
    fn performance_metrics_report_mhz_and_realtime_percentage() {
        let (mhz, percentage) = performance_metrics(16_777_216, std::time::Duration::from_secs(1));

        assert!((mhz - 16.777_216).abs() < f64::EPSILON);
        assert!((percentage - 100.0).abs() < f64::EPSILON);
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
    fn framebuffer_ppm_contains_header_and_rgb_pixels() {
        let mut framebuffer =
            vec![0; crate::gba::Gba::SCREEN_WIDTH * crate::gba::Gba::SCREEN_HEIGHT];
        framebuffer[0] = 0xFF12_3456;

        let mut output = Vec::new();

        write_framebuffer_ppm(&mut output, &framebuffer).unwrap();

        assert!(output.starts_with(b"P6\n240 160\n255\n\x12\x34\x56"));
    }

    #[test]
    fn parses_key_press_and_combines_overlapping_events() {
        let start = parse_key_press(OsString::from("START:100:20")).unwrap();
        let a = parse_key_press(OsString::from("a:0x69:10")).unwrap();

        assert_eq!(
            start,
            KeyPress {
                key: Key::Start,
                start_cycle: 100,
                duration_cycles: 20,
            }
        );
        assert_eq!(scheduled_key_mask(&[start, a], 99), 0);
        assert_eq!(
            scheduled_key_mask(&[start, a], 105),
            Key::Start.mask() | Key::A.mask()
        );
        assert_eq!(scheduled_key_mask(&[start, a], 115), Key::Start.mask());
        assert_eq!(scheduled_key_mask(&[start, a], 120), 0);
    }

    #[test]
    fn rejects_zero_duration_key_press() {
        assert!(matches!(
            parse_key_press(OsString::from("START:100:0")),
            Err(CliError::InvalidKeyPress { .. })
        ));
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
                "--framebuffer-output",
                "frame.ppm",
                "--save",
                "custom.sav",
                "--press-key",
                "START:1200000000:1000000",
                "--strict-cpu",
            ]
            .map(OsString::from),
        )
        .unwrap();

        assert_eq!(config.headless_cycles, Some(1_000_000));
        assert_eq!(config.watch_address, Some(0x0300_22DC));
        assert_eq!(config.framebuffer_output, Some(PathBuf::from("frame.ppm")));
        assert_eq!(config.save_path, Some(PathBuf::from("custom.sav")));
        assert_eq!(config.key_presses.len(), 1);
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

        assert!(matches!(
            error,
            CliError::HeadlessOptionRequiresHeadless {
                option: "--strict-cpu"
            }
        ));
    }
}
