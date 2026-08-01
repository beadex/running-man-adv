mod memory;

pub use memory::MemoryLoadError;

pub(crate) use memory::{BIOS_SIZE, GAME_PAK_ROM_MAX_SIZE};

use memory::{
    EWRAM_SIZE, IO_SIZE, IWRAM_SIZE, Memory, OAM_SIZE, PALETTE_RAM_SIZE, SRAM_SIZE, VRAM_SIZE,
};

const BIOS_START: u32 = 0x0000_0000;
const BIOS_END: u32 = 0x0000_3FFF;

const EWRAM_START: u32 = 0x0200_0000;
const EWRAM_END: u32 = 0x02FF_FFFF;

const IWRAM_START: u32 = 0x0300_0000;
const IWRAM_END: u32 = 0x03FF_FFFF;

const IO_START: u32 = 0x0400_0000;
const IO_END: u32 = 0x04FF_FFFF;

const PALETTE_RAM_START: u32 = 0x0500_0000;
const PALETTE_RAM_END: u32 = 0x05FF_FFFF;

const VRAM_START: u32 = 0x0600_0000;
const VRAM_END: u32 = 0x06FF_FFFF;

const OAM_START: u32 = 0x0700_0000;
const OAM_END: u32 = 0x07FF_FFFF;

const GAME_PAK_ROM_START: u32 = 0x0800_0000;
const GAME_PAK_ROM_END: u32 = 0x0DFF_FFFF;

const SRAM_START: u32 = 0x0E00_0000;
const SRAM_END: u32 = 0x0EFF_FFFF;

#[derive(Debug)]
pub struct Bus {
    memory: Memory,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            memory: Memory::new(),
        }
    }

    pub fn load_bios(&mut self, data: &[u8]) -> Result<(), MemoryLoadError> {
        self.memory.load_bios(data)
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), MemoryLoadError> {
        self.memory.load_rom(data)
    }

    pub fn read8(&self, address: u32) -> u8 {
        match address {
            BIOS_START..=BIOS_END => {
                let offset = address as usize % BIOS_SIZE;
                self.memory.bios[offset]
            }

            EWRAM_START..=EWRAM_END => {
                let offset = (address - EWRAM_START) as usize % EWRAM_SIZE;
                self.memory.ewram[offset]
            }

            IWRAM_START..=IWRAM_END => {
                let offset = (address - IWRAM_START) as usize % IWRAM_SIZE;
                self.memory.iwram[offset]
            }

            IO_START..=IO_END => {
                let offset = (address - IO_START) as usize % IO_SIZE;
                self.memory.io[offset]
            }

            PALETTE_RAM_START..=PALETTE_RAM_END => {
                let offset = (address - PALETTE_RAM_START) as usize % PALETTE_RAM_SIZE;
                self.memory.palette_ram[offset]
            }

            VRAM_START..=VRAM_END => {
                let offset = Self::map_vram_address(address);
                self.memory.vram[offset]
            }

            OAM_START..=OAM_END => {
                let offset = (address - OAM_START) as usize % OAM_SIZE;
                self.memory.oam[offset]
            }

            GAME_PAK_ROM_START..=GAME_PAK_ROM_END => self.read_game_pak_rom(address),

            SRAM_START..=SRAM_END => {
                let offset = (address - SRAM_START) as usize % SRAM_SIZE;
                self.memory.sram[offset]
            }

            _ => self.read_open_bus(address),
        }
    }

    pub fn read16(&self, address: u32) -> u16 {
        let address = address & !1;

        u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
    }

    pub fn read32(&self, address: u32) -> u32 {
        let address = address & !3;

        u32::from_le_bytes([
            self.read8(address),
            self.read8(address.wrapping_add(1)),
            self.read8(address.wrapping_add(2)),
            self.read8(address.wrapping_add(3)),
        ])
    }

    pub fn write8(&mut self, address: u32, value: u8) {
        match address {
            BIOS_START..=BIOS_END => {
                // BIOS is read-only.
            }

            EWRAM_START..=EWRAM_END => {
                let offset = (address - EWRAM_START) as usize % EWRAM_SIZE;
                self.memory.ewram[offset] = value;
            }

            IWRAM_START..=IWRAM_END => {
                let offset = (address - IWRAM_START) as usize % IWRAM_SIZE;
                self.memory.iwram[offset] = value;
            }

            IO_START..=IO_END => {
                let offset = (address - IO_START) as usize % IO_SIZE;
                self.memory.io[offset] = value;
            }

            PALETTE_RAM_START..=PALETTE_RAM_END => {
                self.write_palette8(address, value);
            }

            VRAM_START..=VRAM_END => {
                self.write_vram8(address, value);
            }

            OAM_START..=OAM_END => {
                // Byte writes to OAM are ignored.
            }

            GAME_PAK_ROM_START..=GAME_PAK_ROM_END => {
                // Game Pak ROM is read-only.
            }

            SRAM_START..=SRAM_END => {
                let offset = (address - SRAM_START) as usize % SRAM_SIZE;
                self.memory.sram[offset] = value;
            }

            _ => {
                // Unmapped write: ignored for now.
            }
        }
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        let address = address & !1;
        let bytes = value.to_le_bytes();

        match address {
            BIOS_START..=BIOS_END => {
                // BIOS is read-only.
            }

            EWRAM_START..=EWRAM_END => {
                let offset = (address - EWRAM_START) as usize % EWRAM_SIZE;
                Self::write16_to_slice(&mut self.memory.ewram[..], offset, bytes);
            }

            IWRAM_START..=IWRAM_END => {
                let offset = (address - IWRAM_START) as usize % IWRAM_SIZE;
                Self::write16_to_slice(&mut self.memory.iwram[..], offset, bytes);
            }

            IO_START..=IO_END => {
                let offset = (address - IO_START) as usize % IO_SIZE;
                Self::write16_to_slice(&mut self.memory.io[..], offset, bytes);
            }

            PALETTE_RAM_START..=PALETTE_RAM_END => {
                let offset = (address - PALETTE_RAM_START) as usize % PALETTE_RAM_SIZE;

                Self::write16_to_slice(&mut self.memory.palette_ram[..], offset, bytes);
            }

            VRAM_START..=VRAM_END => {
                let offset = Self::map_vram_address(address);
                Self::write16_to_slice(&mut self.memory.vram[..], offset, bytes);
            }

            OAM_START..=OAM_END => {
                let offset = (address - OAM_START) as usize % OAM_SIZE;
                Self::write16_to_slice(&mut self.memory.oam[..], offset, bytes);
            }

            GAME_PAK_ROM_START..=GAME_PAK_ROM_END => {
                // Game Pak ROM is read-only.
            }

            SRAM_START..=SRAM_END => {
                /*
                 * SRAM is physically 8-bit. For now, break wider writes
                 * into byte writes. Accurate timing is added later.
                 */
                self.write8(address, bytes[0]);
                self.write8(address.wrapping_add(1), bytes[1]);
            }

            _ => {}
        }
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        let address = address & !3;
        let halfwords = [value as u16, (value >> 16) as u16];

        self.write16(address, halfwords[0]);
        self.write16(address.wrapping_add(2), halfwords[1]);
    }

    fn read_game_pak_rom(&self, address: u32) -> u8 {
        if self.memory.game_pak_rom.is_empty() {
            return 0xFF;
        }

        /*
         * All three Game Pak ROM windows address the same maximum
         * 32 MiB cartridge ROM.
         *
         * 0x08000000..0x09FFFFFF
         * 0x0A000000..0x0BFFFFFF
         * 0x0C000000..0x0DFFFFFF
         */
        let offset = (address - GAME_PAK_ROM_START) as usize & 0x01FF_FFFF;

        self.memory
            .game_pak_rom
            .get(offset)
            .copied()
            .unwrap_or(0xFF)
    }

    fn map_vram_address(address: u32) -> usize {
        let mut offset = (address - VRAM_START) as usize & 0x0001_FFFF;

        /*
         * GBA exposes a 128 KiB VRAM address window, but only has
         * 96 KiB of physical VRAM.
         *
         * 0x06018000..0x0601FFFF mirrors 0x06010000..0x06017FFF.
         */
        if offset >= VRAM_SIZE {
            offset -= 32 * 1024;
        }

        offset
    }

    fn write_palette8(&mut self, address: u32, value: u8) {
        /*
         * Palette RAM has a 16-bit data bus. An 8-bit write is
         * replicated into both bytes of the addressed halfword.
         */
        let offset = (address - PALETTE_RAM_START) as usize % PALETTE_RAM_SIZE;
        let aligned_offset = offset & !1;

        self.memory.palette_ram[aligned_offset] = value;
        self.memory.palette_ram[aligned_offset + 1] = value;
    }

    fn write_vram8(&mut self, address: u32, value: u8) {
        /*
         * VRAM also has a 16-bit data bus. An 8-bit write is
         * replicated into both bytes of the addressed halfword.
         */
        let offset = Self::map_vram_address(address);
        let aligned_offset = offset & !1;

        self.memory.vram[aligned_offset] = value;
        self.memory.vram[aligned_offset + 1] = value;
    }

    fn write16_to_slice(memory: &mut [u8], offset: usize, bytes: [u8; 2]) {
        memory[offset] = bytes[0];
        memory[(offset + 1) % memory.len()] = bytes[1];
    }

    fn read_open_bus(&self, _address: u32) -> u8 {
        /*
         * Placeholder.
         *
         * Accurate open-bus behaviour depends on the CPU pipeline and
         * the most recently fetched bus value. Returning zero is enough
         * while building the initial interpreter.
         */
        0
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Bus;

    #[test]
    fn ewram_can_be_read_and_written() {
        let mut bus = Bus::new();

        bus.write32(0x0200_0000, 0x1234_5678);

        assert_eq!(bus.read8(0x0200_0000), 0x78);
        assert_eq!(bus.read16(0x0200_0000), 0x5678);
        assert_eq!(bus.read32(0x0200_0000), 0x1234_5678);
    }

    #[test]
    fn ewram_is_mirrored() {
        let mut bus = Bus::new();

        bus.write8(0x0200_1234, 0x42);

        assert_eq!(bus.read8(0x0204_1234), 0x42);
        assert_eq!(bus.read8(0x0208_1234), 0x42);
    }

    #[test]
    fn iwram_is_mirrored() {
        let mut bus = Bus::new();

        bus.write8(0x0300_1234, 0x7A);

        assert_eq!(bus.read8(0x0300_9234), 0x7A);
        assert_eq!(bus.read8(0x0301_1234), 0x7A);
    }

    #[test]
    fn bios_is_read_only() {
        let mut bus = Bus::new();

        let bios = vec![0xCC; 16 * 1024];
        bus.load_bios(&bios).unwrap();

        assert_eq!(bus.read8(0), 0xCC);

        bus.write8(0, 0x12);

        assert_eq!(bus.read8(0), 0xCC);
    }

    #[test]
    fn rom_is_read_only() {
        let mut bus = Bus::new();

        bus.load_rom(&[0x11, 0x22, 0x33, 0x44]).unwrap();

        assert_eq!(bus.read32(0x0800_0000), 0x4433_2211);

        bus.write32(0x0800_0000, 0xDEAD_BEEF);

        assert_eq!(bus.read32(0x0800_0000), 0x4433_2211);
    }

    #[test]
    fn game_pak_rom_windows_reference_same_rom() {
        let mut bus = Bus::new();

        bus.load_rom(&[0x12, 0x34, 0x56, 0x78]).unwrap();

        assert_eq!(bus.read32(0x0800_0000), 0x7856_3412);
        assert_eq!(bus.read32(0x0A00_0000), 0x7856_3412);
        assert_eq!(bus.read32(0x0C00_0000), 0x7856_3412);
    }

    #[test]
    fn palette_byte_write_is_replicated() {
        let mut bus = Bus::new();

        bus.write8(0x0500_0001, 0xAB);

        assert_eq!(bus.read16(0x0500_0000), 0xABAB);
    }

    #[test]
    fn vram_byte_write_is_replicated() {
        let mut bus = Bus::new();

        bus.write8(0x0600_0001, 0xCD);

        assert_eq!(bus.read16(0x0600_0000), 0xCDCD);
    }

    #[test]
    fn oam_byte_write_is_ignored() {
        let mut bus = Bus::new();

        bus.write8(0x0700_0000, 0xFF);

        assert_eq!(bus.read8(0x0700_0000), 0);
    }

    #[test]
    fn oam_halfword_write_succeeds() {
        let mut bus = Bus::new();

        bus.write16(0x0700_0000, 0x1234);

        assert_eq!(bus.read16(0x0700_0000), 0x1234);
    }

    #[test]
    fn vram_upper_region_is_mirrored() {
        let mut bus = Bus::new();

        bus.write16(0x0601_0000, 0xCAFE);

        assert_eq!(bus.read16(0x0601_8000), 0xCAFE);
    }

    #[test]
    fn sram_is_mirrored() {
        let mut bus = Bus::new();

        bus.write8(0x0E00_1234, 0x55);

        assert_eq!(bus.read8(0x0E01_1234), 0x55);
        assert_eq!(bus.read8(0x0E10_1234), 0x55);
    }

    #[test]
    fn halfword_access_is_little_endian() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0000, 0x1234);

        assert_eq!(bus.read8(0x0200_0000), 0x34);
        assert_eq!(bus.read8(0x0200_0001), 0x12);
    }

    #[test]
    fn word_access_is_little_endian() {
        let mut bus = Bus::new();

        bus.write32(0x0200_0000, 0x1234_5678);

        assert_eq!(bus.read8(0x0200_0000), 0x78);
        assert_eq!(bus.read8(0x0200_0001), 0x56);
        assert_eq!(bus.read8(0x0200_0002), 0x34);
        assert_eq!(bus.read8(0x0200_0003), 0x12);
    }
}
