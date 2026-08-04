# Running Man Advance

Running Man Advance is an experimental Game Boy Advance emulator written in
Rust. It currently uses an ARM7TDMI interpreter and provides an SDL3 frontend,
audio, cartridge saves, RTC support, and a headless mode for diagnostics and
repeatable benchmarks.

The emulator is still under active development. Some games and hardware edge
cases are not supported yet. See [Architecture notes](docs/architecture.md) for
implementation details and known limitations.

## Current capabilities

- ARM and Thumb CPU interpretation, CPU modes, exceptions, IRQ, HALT, and STOP.
- GBA memory map, wait states, timers, DMA, keypad, and PPU timing.
- Video modes 0-4, backgrounds, objects, windows, and blending.
- SDL3 video, keyboard input, frame pacing, and Direct Sound audio output.
- SRAM, Flash 512, Flash 1M, and EEPROM cartridge saves.
- Game Pak GPIO and S3511-compatible RTC for `SIIRTC_V` cartridges.
- Headless execution, strict CPU faults, scripted input, memory watching,
  framebuffer hashing, PPM dumps, and performance statistics.

## Legal notice

This repository does not include a GBA BIOS or commercial ROM images. You must
provide legally obtained dumps from hardware and games that you own.

The BIOS must be exactly 16 KiB (16,384 bytes).

## Requirements

- A Windows, Linux, or macOS development environment.
- [Rust](https://www.rust-lang.org/tools/install) installed through `rustup`.
- A native C/C++ toolchain and CMake, used to build SDL3 from source.

The repository pins the stable Rust channel and installs `rustfmt` and Clippy
through [rust-toolchain.toml](rust-toolchain.toml).

On Windows, install Visual Studio 2022 Build Tools with:

- Desktop development with C++.
- MSVC compiler and Windows SDK.
- CMake tools for Windows, or a separate CMake installation available on
  `PATH`.

Confirm the tools are available:

```text
rustc --version
cargo --version
cmake --version
```

The first build can take several minutes because SDL3 is compiled from source.

## Build

Clone the repository and enter its directory, then build the development
profile:

```text
cargo build
```

The development profile keeps debug symbols and safety checks while using
light optimization. For normal gameplay and all performance measurements, use
the release profile:

```text
cargo build --release
```

The resulting executable is located at:

- Windows: `target\release\running-man-adv.exe`
- Linux/macOS: `target/release/running-man-adv`

## Prepare BIOS and ROM files

The examples below assume this local layout:

```text
running-man-adv/
  firmware/
    gba_bios.bin
  roms/
    game.gba
```

These files are not supplied by the project. They may be stored anywhere as
long as the corresponding paths are passed to the emulator.

## Run interactively

Use Cargo to build and launch the SDL frontend:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba
```

After building, the executable can also be launched directly.

PowerShell:

```text
.\target\release\running-man-adv.exe --bios firmware/gba_bios.bin --rom roms/game.gba
```

Linux/macOS:

```text
./target/release/running-man-adv --bios firmware/gba_bios.bin --rom roms/game.gba
```

### Keyboard controls

| GBA control | Keyboard |
|---|---|
| A | `Z` |
| B | `X` |
| L | `A` |
| R | `S` |
| Start | `Enter` |
| Select | `Backspace` |
| D-pad | Arrow keys |
| Pause/resume | `Space` |
| Quit | `Escape` or close the window |

The SDL window title reports the current FPS and emulation speed.

## Save files and RTC

By default, cartridge data is stored next to the ROM with a `.sav` extension.
For example, `roms/game.gba` uses `roms/game.sav`.

Select a different location with `--save`:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --save saves/game.sav
```

The parent directory of a custom save path must already exist.

Dirty save data is flushed every five emulated seconds and when the emulator
exits normally. Supported save backends are:

- SRAM: 32 KiB.
- Flash 512: 64 KiB.
- Flash 1M: 128 KiB.
- EEPROM: 512 B or 8 KiB.

Legacy 64 KiB SRAM files created by older emulator versions are accepted. The
first 32 KiB is retained and the file is normalized after the next persistent
write. A save file with an incompatible size is rejected instead of silently
truncating data.

Games with a recognized `SIIRTC_V` signature use the host's local date and
time. RTC offset metadata is not persisted separately yet.

## Headless mode

Headless mode runs without creating an SDL window. It is intended for automated
tests, CPU diagnostics, visual regression captures, and benchmarks.

`--headless-cycles` is a budget of emulated GBA clock cycles, not a number of
CPU instructions. The GBA master clock is 16,777,216 cycles per second.

Run a fixed cycle budget:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 120000000 --strict-cpu
```

The final report includes:

- Requested and consumed cycles.
- Host elapsed time, emulated MHz, and percentage of real GBA speed.
- PC, CPSR, CPU state and mode.
- Interrupt and display registers.
- Frame number and a stable FNV-1a framebuffer hash.
- The first CPU fault, when strict mode is enabled.

### Strict CPU mode

Use `--strict-cpu` during validation. It stops the headless run at the first CPU
decode or execution fault instead of continuing with potentially corrupted
state:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 85000000 --strict-cpu
```

`--strict-cpu` requires `--headless-cycles`.

### Watch a memory address

`--watch-address` reads a 32-bit value and reports its final value, change
count, and per-bit rising edges:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 85000000 --watch-address 0x030022DC --strict-cpu
```

Decimal and `0x`-prefixed hexadecimal integers are accepted.

### Dump the final framebuffer

Write the final 240x160 framebuffer as a binary PPM (`P6`) image:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 1200000000 --framebuffer-output final-frame.ppm --strict-cpu
```

PPM is deliberately simple and lossless. Most image editors can open it, or it
can be converted to PNG with an external image tool.

### Script keypad input

Headless input uses repeatable options in this format:

```text
--press-key KEY:START-CYCLE:DURATION-CYCLES
```

Supported key names are `A`, `B`, `SELECT`, `START`, `RIGHT`, `LEFT`, `UP`,
`DOWN`, `R`, and `L`. Names are case-insensitive. Intervals may overlap; active
keys are combined into one keypad state.

Example:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 1800000000 --press-key START:1150000000:5000000 --press-key A:1300000000:5000000 --press-key A:1500000000:5000000 --framebuffer-output scripted-frame.ppm --strict-cpu
```

Cycle values may be decimal or `0x`-prefixed hexadecimal. Duration must be
greater than zero. `--press-key` requires `--headless-cycles`.

## Benchmarking

Always benchmark a release build. Development builds are useful for iteration
but are not representative of emulator performance.

A practical benchmark command is:

```text
cargo run --release -- --bios firmware/gba_bios.bin --rom roms/game.gba --save benchmark.sav --headless-cycles 1200000000 --strict-cpu
```

For meaningful comparisons:

1. Use the same release compiler, BIOS, ROM, cycle budget, and command line.
2. Start every run from an identical save file. The emulator may update the
   selected save during a run, so restore a baseline copy before each sample.
3. Close CPU-intensive background applications and allow the machine to reach
   a stable power state.
4. Run the benchmark more than once and compare the median host time.
5. Confirm the consumed cycle count, final CPU state, frame number, and
   framebuffer hash before comparing speed.

For games using the host RTC, time-dependent guest behavior can legitimately
change the final state or framebuffer hash. Use hashes as a regression oracle
only when the guest workload and its time inputs are deterministic.

The reported real-time percentage is based on the GBA clock:

```text
realtime % = emulated cycles / host seconds / 16,777,216 * 100
```

Values above 100% mean the unthrottled headless core is faster than a real GBA.
Interactive mode remains frame-paced near the GBA refresh rate.

### Performance instrumentation

The `perf-stats` feature attributes host time to major emulator activities and
prints CPU-step, HALT, DMA, and PPU rendering statistics:

```text
cargo run --release --features perf-stats -- --bios firmware/gba_bios.bin --rom roms/game.gba --headless-cycles 1200000000 --strict-cpu
```

Instrumentation affects performance. Use it to locate bottlenecks, not as the
headline benchmark result.

The `cpu-trace` feature prints periodic interactive CPU state diagnostics:

```text
cargo run --release --features cpu-trace -- --bios firmware/gba_bios.bin --rom roms/game.gba
```

## Command-line reference

```text
running-man-adv --bios <bios-file> --rom <rom-file> [options]

--bios <path>                 Legally dumped 16 KiB GBA BIOS
--rom <path>                  GBA ROM image
--save <path>                 Save path; defaults to ROM path with .sav
--headless-cycles <count>     Run without SDL for a fixed cycle budget
--watch-address <address>     Track a 32-bit memory value in headless mode
--framebuffer-output <path>   Write the final framebuffer as binary PPM
--press-key <key>:<start>:<duration>
                              Schedule a headless key press; repeatable
--strict-cpu                  Stop at the first headless CPU fault
-h, --help                    Print CLI help
```

## Tests and code quality

Run the complete test suite:

```text
cargo test --all-features
```

Check all targets and optional features:

```text
cargo check --all-targets --all-features
```

Verify formatting and run Clippy with warnings treated as errors:

```text
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Apply Rust formatting:

```text
cargo fmt
```

## Troubleshooting

### The game runs slowly

Use `cargo run --release`, not plain `cargo run`. The development profile is
optimized for debugging and iteration rather than maximum speed.

### The first build takes a long time

This is expected. SDL3 is built from source, and release builds use thin LTO
with a single code-generation unit. Incremental builds are much faster.

### SDL cannot initialize video or audio

Update the graphics/audio drivers and make sure the process is running in a
desktop session. Audio initialization failure is non-fatal; the emulator can
continue in video-only mode. Headless mode does not require an SDL window or
host PCM output.

### The BIOS is rejected

Verify that the BIOS dump is exactly 16,384 bytes and is a real GBA BIOS rather
than a ROM, compressed archive, or emulator-specific replacement.

### An existing save is rejected

The save size must match the backend detected from the ROM. Do not resize the
file manually. Keep a backup, then verify that the ROM and save belong to the
same game and emulator configuration.

### Benchmark results vary

Use an identical save baseline, repeat the run, compare the median, and avoid
background load. RTC-enabled games can also vary with the host clock.

## Project layout

```text
src/lib.rs             Reusable emulator core crate
src/main.rs            CLI and headless runner
src/gba.rs             Top-level machine scheduler
src/bus/               Memory bus and GBA hardware components
src/cpu/               ARM and Thumb interpreter
src/frontend/sdl.rs    Interactive SDL3 frontend
src/save_file.rs       Persistent cartridge save handling
docs/architecture.md   Detailed architecture and milestone notes
```

## Development status

The emulator can boot the official BIOS and run tested commercial software,
but compatibility is not complete. Important remaining accuracy work includes
PSG channels 1-4, cycle-exact audio, exact EEPROM busy timing, non-RTC GPIO
devices, more complete cartridge detection, RTC persistence details, and other
hardware edge cases.

When investigating a compatibility problem, prefer a short reproducible
headless command with `--strict-cpu`, a fixed save, a framebuffer hash or PPM,
and any relevant watched address.
