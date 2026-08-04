# Game Boy Advance Emulator
## Architecture Notes

**Language:** Rust  
**Primary reference:** GBATEK  
**Target CPU:** ARM7TDMI / ARMv4T  
**Long-term goals:**
- Build an accurate Game Boy Advance emulator.
- Start with a correct CPU interpreter.
- Later reuse the CPU semantics for a dynamic recompiler.
- Use modern graphics APIs such as Vulkan and Direct3D 12 for rendering backends.

> **Status note:** The detailed milestone inventory later in this document is
> historical and currently lags the implementation. The code now includes ARM
> and Thumb execution, banked CPU modes, exceptions/IRQ, timers, DMA, PPU
> timing, video modes 0/2/3/4, objects, blending, keypad/HALT, and SDL output.
> Cartridge save selection by ROM signature and 128 KiB Flash 1M command
> emulation are also implemented; EEPROM and save-file persistence are not.
> Keep this note until the full architecture document is rewritten.

## Headless validation

The emulator can run without SDL for deterministic boot diagnostics:

```text
cargo run -- --bios firmware/gba_bios.bin --rom roms/test.gba \
  --headless-cycles 85000000 --watch-address 0x030022DC
```

The final dump includes CPU state, CPSR/mode, interrupt registers, PPU state,
the BIOS IRQ handler pointer, a stable framebuffer hash, and the optional
watched memory value. A watched value also reports its number of changes and
per-bit rising-edge counts during the run.

Validation runs can stop at the first CPU decode or execution fault instead of
logging the error and continuing with corrupted state:

```text
cargo run -- --bios firmware/gba_bios.bin --rom roms/test.gba \
  --headless-cycles 85000000 --strict-cpu
```

---

# 1. Development Strategy

The project is developed incrementally:

1. Rust project skeleton
2. Address bus and memory regions
3. BIOS and ROM loading
4. ARM instruction classification
5. ARM condition evaluation
6. Detailed ARM instruction decoding
7. ARM instruction execution
8. Thumb instruction decoding and execution
9. Exceptions and CPU modes
10. Interrupts
11. Timers
12. DMA
13. PPU
14. Audio
15. Save-memory hardware
16. Timing and accuracy improvements
17. Dynamic recompiler

The current priority is correctness and testability, not performance.

The interpreter must become a trusted reference implementation before a
dynamic recompiler is introduced.

---

# 2. Current Source Layout

```text
src/
├── main.rs
├── loader.rs
├── bus/
│   ├── mod.rs
│   └── memory.rs
└── cpu/
    ├── mod.rs
    ├── cpsr.rs
    ├── registers.rs
    └── arm/
        ├── mod.rs
        └── decoder.rs
```

---

# 3. Module Responsibilities

## `main.rs`

Responsibilities:

- Parse command-line arguments.
- Receive BIOS and ROM paths.
- Load BIOS and ROM images through the loader.
- Initialize the bus and CPU.
- Set the initial CPU state.
- Start CPU execution.

It must not contain instruction-decoding logic or memory-map logic.

## `loader.rs`

Responsibilities:

- Read files from the host filesystem.
- Validate BIOS size.
- Validate ROM size.
- Parse the GBA ROM header.
- Validate the fixed header byte.
- Validate the ROM header checksum.
- Expose BIOS and ROM bytes to the emulator core.

The loader must not:

- Execute CPU instructions.
- Perform address translation.
- Emulate cartridge hardware.
- Own emulator runtime state.

Keeping filesystem access outside the emulator core makes it possible to reuse
the core from CLI, desktop, mobile, WebAssembly, tests, and future frontends.

## `bus/`

Responsibilities:

- Decode CPU addresses.
- Route accesses to the correct memory region.
- Implement memory mirroring.
- Enforce read-only regions.
- Implement access-width-specific behavior.
- Provide `read8`, `read16`, `read32`, `write8`, `write16`, and `write32`.

The bus must not:

- Parse files.
- Decode ARM or Thumb instructions.
- Implement CPU-specific unaligned-load rotation.
- Render graphics.

## `cpu/`

Responsibilities:

- Store ARM7TDMI execution state.
- Fetch instructions from the bus.
- Track ARM or Thumb execution state.
- Evaluate ARM condition codes.
- Decode and execute instructions.
- Update registers and CPSR.
- Later handle CPU modes, exceptions, and pipeline behavior.

The CPU must not know whether an address maps to BIOS, RAM, ROM, or MMIO.

## `cpu/arm/decoder.rs`

Current responsibilities:

- Extract the ARM condition field.
- Evaluate conditions against CPSR flags.
- Classify ARM instructions into major instruction families.
- Detect overlapping special encodings before broad major groups.

It does not yet parse detailed operands or execute instructions.

---

# 4. GBA Memory Map

| Region | Address range | Physical size |
|---|---|---:|
| BIOS | `0x00000000-0x00003FFF` | 16 KiB |
| EWRAM | `0x02000000-0x02FFFFFF` | 256 KiB mirrored |
| IWRAM | `0x03000000-0x03FFFFFF` | 32 KiB mirrored |
| I/O | `0x04000000-0x04FFFFFF` | 1 KiB currently backed by storage |
| Palette RAM | `0x05000000-0x05FFFFFF` | 1 KiB mirrored |
| VRAM | `0x06000000-0x06FFFFFF` | 96 KiB with special mirroring |
| OAM | `0x07000000-0x07FFFFFF` | 1 KiB mirrored |
| Game Pak ROM | `0x08000000-0x0DFFFFFF` | Up to 32 MiB |
| Cartridge save | `0x0E000000-0x0EFFFFFF` | 64 KiB SRAM or banked 128 KiB Flash 1M |

Current bus behavior:

- Little-endian memory access.
- BIOS is read-only.
- Game Pak ROM is read-only.
- EWRAM and IWRAM are mirrored.
- Palette RAM byte writes replicate the byte across a halfword.
- VRAM byte writes replicate the byte across a halfword.
- OAM byte writes are ignored.
- OAM halfword and word writes are supported.
- The three Game Pak ROM windows reference the same ROM image.
- `FLASH1M_V` ROM signatures select a banked 128 KiB Flash backend with ID,
  byte-program, sector/chip erase, and bank-select commands.
- Unknown save signatures currently fall back to 64 KiB SRAM.
- Non-volatile cartridge data survives an emulated machine reset, but is not
  yet persisted to a host save file.
- Unmapped reads currently return a placeholder open-bus value.
- Unmapped writes are currently ignored.

Important design rule:

> Access-width behavior must not be implemented by decomposing every wider
> access into byte accesses.

For example, `write16` cannot be implemented as two calls to `write8`, because
Palette RAM, VRAM, and OAM have special byte-write behavior.

---

# 5. BIOS and ROM Loading

The emulator accepts:

```text
--bios <path>
--rom <path>
```

Example:

```bash
cargo run -- --bios firmware/gba_bios.bin --rom roms/test.gba
```

BIOS validation:

- Exact size: 16 KiB.

ROM validation:

- Minimum size sufficient for the GBA header.
- Maximum size: 32 MiB.
- Fixed byte at `0xB2` must be `0x96`.
- Header checksum at `0xBD` must match the checksum computed over
  `0xA0..=0xBC`.

Parsed ROM metadata:

- Title
- Game code
- Maker code
- Software version
- Header checksum

BIOS and commercial ROM images must not be committed to the repository.

Recommended `.gitignore` entries:

```gitignore
/target
/firmware
/roms
*.gba
*.bin
*.sav
```

---

# 6. CPU State

The CPU currently stores:

```text
Cpu
├── Registers
├── CPSR
├── ARM/Thumb execution state
└── halted flag
```

The general register file currently contains:

```text
R0-R12  General-purpose registers
R13     Stack Pointer
R14     Link Register
R15     Program Counter
```

Banked registers are not implemented yet.

The current sequential execution model advances:

```text
ARM instruction  -> PC + 4
Thumb instruction -> PC + 2
```

This is temporary. ARM pipeline-visible PC behavior will be modeled later.

---

# 7. CPSR Representation

CPSR is represented by a dedicated `Cpsr` type instead of a raw `u32`.

```rust
pub struct Cpsr {
    value: u32,
}
```

The currently exposed condition flags are:

| Bit | Flag | Meaning |
|---:|---|---|
| 31 | N | Negative |
| 30 | Z | Zero |
| 29 | C | Carry |
| 28 | V | Overflow |

The API provides:

```rust
cpsr.raw()
cpsr.set_raw(value)

cpsr.negative()
cpsr.zero()
cpsr.carry()
cpsr.overflow()

cpsr.set_negative(value)
cpsr.set_zero(value)
cpsr.set_carry(value)
cpsr.set_overflow(value)
cpsr.set_nzcv(n, z, c, v)
```

`Registers::cpsr()` returns a `Cpsr`, not a `u32`.

Therefore, tests and call sites that need the raw integer value must use:

```rust
registers.cpsr().raw()
```

Correct zero-initialization test:

```rust
assert_eq!(registers.cpsr().raw(), 0);
```

The CPU module re-exports the type using:

```rust
pub use self::cpsr::Cpsr;
```

The `Cpsr` struct itself must be public:

```rust
pub struct Cpsr {
    value: u32,
}
```

Within sibling CPU modules, direct internal imports may use:

```rust
use super::cpsr::Cpsr;
```

---

# 8. ARM Instruction Classification

ARM instructions are currently classified into:

```rust
pub enum ArmInstructionKind {
    BranchExchange,
    Multiply,
    MultiplyLong,
    SingleDataSwap,
    HalfwordDataTransfer,
    DataProcessing,
    SingleDataTransfer,
    BlockDataTransfer,
    Branch,
    CoprocessorDataTransfer,
    CoprocessorDataOperation,
    CoprocessorRegisterTransfer,
    SoftwareInterrupt,
    Undefined,
}
```

Classification order matters because ARM encodings overlap.

The decoder checks specific patterns before broad major groups:

```text
BX
↓
Multiply
↓
Multiply Long
↓
Single Data Swap
↓
Halfword and Signed Data Transfer
↓
Major instruction group
```

A classifier based only on bits `27..25` would incorrectly classify instructions
such as `MUL` as data-processing instructions.

Current known simplification:

- `MRS` and `MSR` are still grouped under `DataProcessing`.
- Detailed operand decoding has not yet been implemented.
- Coprocessor instructions are classified even though the GBA ARM7TDMI does not
  provide a general-purpose coprocessor for normal software use.

---

# 9. ARM Condition Codes

The condition field is stored in bits `31..28`.

Supported condition enum:

```rust
pub enum ArmCondition {
    Equal,
    NotEqual,
    CarrySet,
    CarryClear,
    Minus,
    Plus,
    Overflow,
    NoOverflow,
    UnsignedHigher,
    UnsignedLowerOrSame,
    SignedGreaterOrEqual,
    SignedLessThan,
    SignedGreaterThan,
    SignedLessOrEqual,
    Always,
    Never,
}
```

Condition evaluation:

| Condition | Expression |
|---|---|
| EQ | `Z` |
| NE | `!Z` |
| CS/HS | `C` |
| CC/LO | `!C` |
| MI | `N` |
| PL | `!N` |
| VS | `V` |
| VC | `!V` |
| HI | `C && !Z` |
| LS | `!C || Z` |
| GE | `N == V` |
| LT | `N != V` |
| GT | `!Z && N == V` |
| LE | `Z || N != V` |
| AL | `true` |
| NV | `false` |

Signed comparisons must combine `N` and `V`.

The sign flag alone is insufficient because signed overflow can make the result
sign differ from the mathematically correct comparison result.

Condition `0b1111` is currently treated as `Never`, which is appropriate for the
ARMv4T target used by the ARM7TDMI.

---

# 10. Current ARM Step Flow

The current simplified ARM execution flow is:

```text
fetch instruction
        ↓
extract condition
        ↓
advance sequential PC
        ↓
evaluate condition against CPSR
        ├── failed -> skip instruction
        └── passed -> classify instruction
```

A failed condition has no instruction side effects but still advances execution.

Current `step_arm` behavior:

- Fetches one 32-bit instruction.
- Reads the instruction condition.
- Evaluates it using CPSR.
- Advances PC by four.
- Skips classification when the condition fails.
- Classifies the instruction when the condition passes.
- Returns a placeholder cycle count.

Instruction execution is not implemented yet.

---

# 11. Testing Strategy

Every architectural behavior should have focused unit tests.

Current test categories include:

## Memory

- Little-endian reads and writes.
- EWRAM mirroring.
- IWRAM mirroring.
- VRAM mirroring.
- SRAM mirroring.
- BIOS read-only behavior.
- ROM read-only behavior.
- Palette byte-write replication.
- VRAM byte-write replication.
- OAM byte-write rejection.
- OAM halfword writes.

## Loader

- Valid ROM header parsing.
- Invalid fixed-value rejection.
- Invalid checksum rejection.
- ASCII field decoding.
- BIOS size validation.
- ROM size validation.

## CPSR

- Flags clear by default.
- Individual flag updates.
- Updating one flag preserves unrelated bits.
- Raw CPSR conversion.
- Combined NZCV updates.

## ARM classification

- Branch exchange.
- Multiply.
- Multiply long.
- Swap.
- Halfword and signed transfers.
- Data processing.
- Single data transfer.
- Block transfer.
- Branch.
- Coprocessor groups.
- Software interrupt.
- Undefined overlapping encodings.
- Classification independence from condition code.

## Condition evaluation

All sixteen condition codes are tested.

An exhaustive test checks:

```text
16 conditions × 16 NZCV combinations = 256 cases
```

This exhaustive test should remain in the project as a regression test.

---

# 12. Known Limitations

The emulator does not yet model:

- ARM or Thumb instruction execution.
- Detailed data-processing decoding.
- Barrel-shifter behavior.
- Pipeline-visible PC semantics.
- Pipeline refill.
- CPU modes.
- Banked registers.
- SPSRs.
- Exceptions.
- IRQ and FIQ.
- BIOS read protection.
- Accurate open-bus behavior.
- Bus wait states.
- Sequential versus non-sequential access.
- `WAITCNT`.
- Game Pak prefetch.
- Accurate cycle timing.
- MMIO side effects.
- DMA.
- Timers.
- PPU.
- Audio.
- EEPROM.
- Game Pak GPIO.
- Complete cartridge hardware detection beyond SRAM and `FLASH1M_V`.
- Dynamic recompilation.

These limitations are intentional at the current milestone.

---

# 13. Design Principles

## Separation of responsibilities

```text
Loader -> reads and validates files
Bus    -> maps addresses and performs memory accesses
CPU    -> executes ARM7TDMI instructions
PPU    -> generates GBA video output
Frontend -> presents audio, video, input, and debugging UI
```

## Interpreter first

The interpreter is the future correctness oracle for the dynamic recompiler.

The dynarec must be checked against interpreter behavior using identical CPU
state and memory inputs.

## Keep semantics reusable

Detailed instruction decoding and instruction semantics should not become
tightly coupled to the interpreter loop.

Future structure may separate:

```text
decode
execute semantics
interpreter dispatch
IR lowering
native code generation
```

## Bus does not implement CPU-specific unaligned semantics

The bus exposes memory accesses.

Instructions such as ARM `LDR` are responsible for architecture-specific rotate
behavior on unaligned addresses.

## Prefer typed decoded instructions

Raw bit manipulation should be concentrated in decoder modules.

Execution code should eventually receive typed decoded instructions rather than
repeatedly extracting fields from a raw `u32`.

## Add regression tests with every instruction family

Every implemented instruction should include:

- Normal-case tests.
- Boundary-value tests.
- Condition-failed tests.
- Flag-update tests.
- PC destination tests where applicable.
- Unaligned-memory tests where applicable.

---

# 14. Next Milestone

The next milestone is the detailed decoder for ARM data-processing instructions.

It should decode:

```text
condition
immediate/register operand form
opcode
S bit
Rn
Rd
Operand2
```

Planned typed representation:

```rust
pub struct DataProcessingInstruction {
    pub condition: ArmCondition,
    pub opcode: DataProcessingOpcode,
    pub set_flags: bool,
    pub rn: RegisterIndex,
    pub rd: RegisterIndex,
    pub operand2: Operand2,
}
```

Possible opcode enum:

```rust
pub enum DataProcessingOpcode {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}
```

`Operand2` must later represent:

- Rotated immediate.
- Immediate shift.
- Register-controlled shift.
- Logical shift left.
- Logical shift right.
- Arithmetic shift right.
- Rotate right.
- Rotate right extended.

The recommended next implementation step is decoding these fields without
executing them.
