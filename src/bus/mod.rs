use std::error::Error;
use std::fmt;

mod interrupt;
mod io;
mod memory;

pub use self::interrupt::{InterruptController, InterruptSource};

pub use self::io::IoRegisters;

pub(crate) const GAME_PAK_ROM_MAX_SIZE: usize = 32 * 1024 * 1024;

const BIOS_BASE: u32 = 0x0000_0000;
pub(crate) const BIOS_SIZE: usize = 0x0000_4000;

const EWRAM_BASE: u32 = 0x0200_0000;
const EWRAM_SIZE: usize = 0x0004_0000;

const IWRAM_BASE: u32 = 0x0300_0000;
const IWRAM_SIZE: usize = 0x0000_8000;

const PALETTE_BASE: u32 = 0x0500_0000;
const PALETTE_SIZE: usize = 0x0000_0400;

const VRAM_BASE: u32 = 0x0600_0000;
const VRAM_SIZE: usize = 0x0001_8000;

const OAM_BASE: u32 = 0x0700_0000;
const OAM_SIZE: usize = 0x0000_0400;

const ROM_BASE: u32 = 0x0800_0000;
const ROM_END: u32 = 0x0DFF_FFFF;

const SRAM_BASE: u32 = 0x0E00_0000;
const SRAM_SIZE: usize = 0x0001_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusLoadError {
    InvalidBiosSize { expected: usize, actual: usize },

    RomTooLarge { maximum: usize, actual: usize },
}

impl fmt::Display for BusLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBiosSize { expected, actual } => {
                write!(
                    formatter,
                    "invalid BIOS size: expected {expected} bytes, got {actual} bytes"
                )
            }

            Self::RomTooLarge { maximum, actual } => {
                write!(
                    formatter,
                    "ROM is too large: maximum {maximum} bytes, got {actual} bytes"
                )
            }
        }
    }
}

impl Error for BusLoadError {}

#[derive(Debug, Clone)]
pub struct Bus {
    bios: Box<[u8; BIOS_SIZE]>,
    ewram: Box<[u8; EWRAM_SIZE]>,
    iwram: Box<[u8; IWRAM_SIZE]>,

    io: IoRegisters,

    palette: Box<[u8; PALETTE_SIZE]>,
    vram: Box<[u8; VRAM_SIZE]>,
    oam: Box<[u8; OAM_SIZE]>,

    rom: Vec<u8>,
    sram: Box<[u8; SRAM_SIZE]>,
}

impl Bus {
    pub const REG_IE: u32 = IoRegisters::BASE + IoRegisters::IE_OFFSET;

    pub const REG_IF: u32 = IoRegisters::BASE + IoRegisters::IF_OFFSET;

    pub const REG_IME: u32 = IoRegisters::BASE + IoRegisters::IME_OFFSET;

    pub fn new() -> Self {
        Self {
            bios: Box::new([0; BIOS_SIZE]),
            ewram: Box::new([0; EWRAM_SIZE]),
            iwram: Box::new([0; IWRAM_SIZE]),

            io: IoRegisters::new(),

            palette: Box::new([0; PALETTE_SIZE]),

            vram: Box::new([0; VRAM_SIZE]),

            oam: Box::new([0; OAM_SIZE]),

            rom: Vec::new(),

            sram: Box::new([0; SRAM_SIZE]),
        }
    }

    pub fn reset(&mut self) {
        self.ewram.fill(0);
        self.iwram.fill(0);
        self.io.reset();
        self.palette.fill(0);
        self.vram.fill(0);
        self.oam.fill(0);
        self.sram.fill(0);

        /*
         * BIOS and cartridge ROM are preserved across reset.
         */
    }

    pub fn load_bios(&mut self, bios: &[u8]) -> Result<(), BusLoadError> {
        if bios.len() != BIOS_SIZE {
            return Err(BusLoadError::InvalidBiosSize {
                expected: BIOS_SIZE,
                actual: bios.len(),
            });
        }

        self.bios.copy_from_slice(bios);

        Ok(())
    }

    pub fn load_rom(&mut self, rom: &[u8]) -> Result<(), BusLoadError> {
        /*
         * GBA cartridge ROM address space:
         * 0x08000000..=0x0DFFFFFF
         *
         * Three wait-state regions mirror the cartridge.
         * Maximum physical image size commonly modeled here is 32 MiB.
         */
        const MAX_ROM_SIZE: usize = 32 * 1024 * 1024;

        if rom.len() > MAX_ROM_SIZE {
            return Err(BusLoadError::RomTooLarge {
                maximum: MAX_ROM_SIZE,
                actual: rom.len(),
            });
        }

        self.rom.clear();
        self.rom.extend_from_slice(rom);

        Ok(())
    }

    pub const fn io(&self) -> &IoRegisters {
        &self.io
    }

    pub fn io_mut(&mut self) -> &mut IoRegisters {
        &mut self.io
    }

    pub const fn interrupt_controller(&self) -> &InterruptController {
        self.io.interrupts()
    }

    pub fn interrupt_controller_mut(&mut self) -> &mut InterruptController {
        self.io.interrupts_mut()
    }

    pub fn request_interrupt(&mut self, source: InterruptSource) {
        self.io.request_interrupt(source);
    }

    pub const fn irq_line(&self) -> bool {
        self.io.irq_line()
    }

    pub fn read8(&self, address: u32) -> u8 {
        match address {
            BIOS_BASE..=0x0000_3FFF => {
                let offset = (address - BIOS_BASE) as usize;

                self.bios[offset]
            }

            0x0200_0000..=0x02FF_FFFF => {
                let offset = mirror_offset(address, EWRAM_BASE, EWRAM_SIZE);

                self.ewram[offset]
            }

            0x0300_0000..=0x03FF_FFFF => {
                let offset = mirror_offset(address, IWRAM_BASE, IWRAM_SIZE);

                self.iwram[offset]
            }

            address if IoRegisters::contains_address(address) => {
                let offset = IoRegisters::address_to_offset(address);

                self.io.read8(offset)
            }

            0x0500_0000..=0x05FF_FFFF => {
                let offset = mirror_offset(address, PALETTE_BASE, PALETTE_SIZE);

                self.palette[offset]
            }

            0x0600_0000..=0x06FF_FFFF => {
                let offset = vram_offset(address);

                self.vram[offset]
            }

            0x0700_0000..=0x07FF_FFFF => {
                let offset = mirror_offset(address, OAM_BASE, OAM_SIZE);

                self.oam[offset]
            }

            ROM_BASE..=ROM_END => self.read_rom8(address),

            0x0E00_0000..=0x0EFF_FFFF => {
                let offset = mirror_offset(address, SRAM_BASE, SRAM_SIZE);

                self.sram[offset]
            }

            _ => 0,
        }
    }

    pub fn write8(&mut self, address: u32, value: u8) {
        match address {
            /*
             * BIOS is read-only.
             */
            BIOS_BASE..=0x0000_3FFF => {}

            0x0200_0000..=0x02FF_FFFF => {
                let offset = mirror_offset(address, EWRAM_BASE, EWRAM_SIZE);

                self.ewram[offset] = value;
            }

            0x0300_0000..=0x03FF_FFFF => {
                let offset = mirror_offset(address, IWRAM_BASE, IWRAM_SIZE);

                self.iwram[offset] = value;
            }

            address if IoRegisters::contains_address(address) => {
                let offset = IoRegisters::address_to_offset(address);

                self.io.write8(offset, value);
            }

            0x0500_0000..=0x05FF_FFFF => {
                let offset = mirror_offset(address, PALETTE_BASE, PALETTE_SIZE);

                self.palette[offset] = value;
            }

            0x0600_0000..=0x06FF_FFFF => {
                let offset = vram_offset(address);

                self.vram[offset] = value;
            }

            0x0700_0000..=0x07FF_FFFF => {
                let offset = mirror_offset(address, OAM_BASE, OAM_SIZE);

                self.oam[offset] = value;
            }

            /*
             * Game Pak ROM is read-only.
             */
            ROM_BASE..=ROM_END => {}

            0x0E00_0000..=0x0EFF_FFFF => {
                let offset = mirror_offset(address, SRAM_BASE, SRAM_SIZE);

                self.sram[offset] = value;
            }

            _ => {}
        }
    }

    pub fn read16(&self, address: u32) -> u16 {
        let aligned = address & !1;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            return self.io.read16(offset);
        }

        let low = self.read8(aligned);
        let high = self.read8(aligned.wrapping_add(1));

        u16::from_le_bytes([low, high])
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        let aligned = address & !1;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            self.io.write16(offset, value);

            return;
        }

        let [low, high] = value.to_le_bytes();

        self.write8(aligned, low);

        self.write8(aligned.wrapping_add(1), high);
    }

    pub fn read32(&self, address: u32) -> u32 {
        let aligned = address & !3;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            return self.io.read32(offset);
        }

        let b0 = self.read8(aligned);

        let b1 = self.read8(aligned.wrapping_add(1));

        let b2 = self.read8(aligned.wrapping_add(2));

        let b3 = self.read8(aligned.wrapping_add(3));

        u32::from_le_bytes([b0, b1, b2, b3])
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        let aligned = address & !3;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            self.io.write32(offset, value);

            return;
        }

        let bytes = value.to_le_bytes();

        for (index, byte) in bytes.into_iter().enumerate() {
            self.write8(aligned.wrapping_add(index as u32), byte);
        }
    }

    fn read_rom8(&self, address: u32) -> u8 {
        if self.rom.is_empty() {
            return 0;
        }

        /*
         * Wait-state regions:
         *
         * 0x08000000
         * 0x0A000000
         * 0x0C000000
         *
         * mirror the same physical ROM.
         */
        let offset = ((address - ROM_BASE) & 0x01FF_FFFF) as usize;

        self.rom.get(offset).copied().unwrap_or(0)
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

fn mirror_offset(address: u32, base: u32, size: usize) -> usize {
    ((address - base) as usize) % size
}

fn vram_offset(address: u32) -> usize {
    /*
     * GBA VRAM is 96 KiB.
     *
     * The upper 32 KiB portion is mirrored within the
     * 128 KiB VRAM address window.
     */
    let offset = ((address - VRAM_BASE) & 0x1_FFFF) as usize;

    if offset >= VRAM_SIZE {
        offset - 0x8000
    } else {
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::{Bus, BusLoadError, InterruptController, InterruptSource};

    #[test]
    fn bios_is_loaded_and_read_only() {
        let mut bus = Bus::new();

        let mut bios = vec![0u8; 0x4000];

        bios[8..12].copy_from_slice(&0xE1B0_F00Eu32.to_le_bytes());

        bus.load_bios(&bios).unwrap();

        assert_eq!(bus.read32(0x0000_0008), 0xE1B0_F00E);

        bus.write32(0x0000_0008, 0xDEAD_BEEF);

        assert_eq!(bus.read32(0x0000_0008), 0xE1B0_F00E);
    }

    #[test]
    fn invalid_bios_size_is_rejected() {
        let mut bus = Bus::new();

        assert_eq!(
            bus.load_bios(&[0; 16]),
            Err(BusLoadError::InvalidBiosSize {
                expected: 0x4000,
                actual: 16,
            })
        );
    }

    #[test]
    fn ewram_is_mirrored() {
        let mut bus = Bus::new();

        bus.write32(0x0200_0000, 0x1234_5678);

        assert_eq!(bus.read32(0x0204_0000), 0x1234_5678);
    }

    #[test]
    fn iwram_is_mirrored() {
        let mut bus = Bus::new();

        bus.write32(0x0300_0000, 0x89AB_CDEF);

        assert_eq!(bus.read32(0x0300_8000), 0x89AB_CDEF);
    }

    #[test]
    fn ie_is_mapped() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_IE, 0x1234);

        assert_eq!(
            bus.read16(Bus::REG_IE),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn if_is_write_one_to_clear() {
        let mut bus = Bus::new();

        bus.request_interrupt(InterruptSource::VBlank);

        bus.request_interrupt(InterruptSource::Timer0);

        bus.write16(Bus::REG_IF, InterruptSource::VBlank.mask());

        assert_eq!(bus.read16(Bus::REG_IF), InterruptSource::Timer0.mask());
    }

    #[test]
    fn ime_is_mapped() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_IME, 1);

        assert_eq!(bus.read16(Bus::REG_IME), 1);
    }

    #[test]
    fn irq_line_comes_from_io_controller() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        bus.write16(Bus::REG_IME, 1);

        bus.request_interrupt(InterruptSource::Timer0);

        assert!(bus.irq_line());

        bus.write16(Bus::REG_IF, InterruptSource::Timer0.mask());

        assert!(!bus.irq_line());
    }

    #[test]
    fn byte_access_to_ie_is_little_endian() {
        let mut bus = Bus::new();

        bus.write8(Bus::REG_IE, 0x34);

        bus.write8(Bus::REG_IE + 1, 0x12);

        assert_eq!(
            bus.read16(Bus::REG_IE),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn word_access_to_ie_and_if_uses_register_semantics() {
        let mut bus = Bus::new();

        bus.request_interrupt(InterruptSource::Timer0);

        let value =
            (InterruptSource::Timer0.mask() as u32) << 16 | InterruptSource::Timer0.mask() as u32;

        bus.write32(Bus::REG_IE, value);

        assert_eq!(bus.read16(Bus::REG_IE), InterruptSource::Timer0.mask());

        assert_eq!(bus.read16(Bus::REG_IF), 0);
    }
}
