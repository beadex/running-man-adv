use std::error::Error;
use std::fmt;

mod dma;
mod interrupt;
mod io;
mod memory;
mod timer;

pub use self::dma::{
    DMA_CHANNEL_COUNT, DmaAddressControl, DmaChannel, DmaChannelIndex, DmaControl, DmaController,
    DmaStartTiming, DmaTransferCompletion, DmaTransferRequest, DmaTransferWidth,
};

pub use self::interrupt::{InterruptController, InterruptSource};

pub use self::io::IoRegisters;

pub use self::timer::{TIMER_COUNT, Timer, TimerControl, TimerController, TimerIndex};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRunResult {
    pub channel: DmaChannelIndex,
    pub transferred_units: u32,
    pub cycles: u32,
}

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
    pub const REG_TM0CNT_L: u32 = IoRegisters::BASE + IoRegisters::TM0CNT_L_OFFSET;

    pub const REG_TM0CNT_H: u32 = IoRegisters::BASE + IoRegisters::TM0CNT_H_OFFSET;

    pub const REG_TM1CNT_L: u32 = IoRegisters::BASE + IoRegisters::TM1CNT_L_OFFSET;

    pub const REG_TM1CNT_H: u32 = IoRegisters::BASE + IoRegisters::TM1CNT_H_OFFSET;

    pub const REG_TM2CNT_L: u32 = IoRegisters::BASE + IoRegisters::TM2CNT_L_OFFSET;

    pub const REG_TM2CNT_H: u32 = IoRegisters::BASE + IoRegisters::TM2CNT_H_OFFSET;

    pub const REG_TM3CNT_L: u32 = IoRegisters::BASE + IoRegisters::TM3CNT_L_OFFSET;

    pub const REG_TM3CNT_H: u32 = IoRegisters::BASE + IoRegisters::TM3CNT_H_OFFSET;

    pub const REG_DMA0SAD: u32 = IoRegisters::BASE + IoRegisters::DMA0SAD_OFFSET;

    pub const REG_DMA0DAD: u32 = IoRegisters::BASE + IoRegisters::DMA0DAD_OFFSET;

    pub const REG_DMA0CNT_L: u32 = IoRegisters::BASE + IoRegisters::DMA0CNT_L_OFFSET;

    pub const REG_DMA0CNT_H: u32 = IoRegisters::BASE + IoRegisters::DMA0CNT_H_OFFSET;

    pub const REG_DMA1SAD: u32 = IoRegisters::BASE + IoRegisters::DMA1SAD_OFFSET;

    pub const REG_DMA1DAD: u32 = IoRegisters::BASE + IoRegisters::DMA1DAD_OFFSET;

    pub const REG_DMA1CNT_L: u32 = IoRegisters::BASE + IoRegisters::DMA1CNT_L_OFFSET;

    pub const REG_DMA1CNT_H: u32 = IoRegisters::BASE + IoRegisters::DMA1CNT_H_OFFSET;

    pub const REG_DMA2SAD: u32 = IoRegisters::BASE + IoRegisters::DMA2SAD_OFFSET;

    pub const REG_DMA2DAD: u32 = IoRegisters::BASE + IoRegisters::DMA2DAD_OFFSET;

    pub const REG_DMA2CNT_L: u32 = IoRegisters::BASE + IoRegisters::DMA2CNT_L_OFFSET;

    pub const REG_DMA2CNT_H: u32 = IoRegisters::BASE + IoRegisters::DMA2CNT_H_OFFSET;

    pub const REG_DMA3SAD: u32 = IoRegisters::BASE + IoRegisters::DMA3SAD_OFFSET;

    pub const REG_DMA3DAD: u32 = IoRegisters::BASE + IoRegisters::DMA3DAD_OFFSET;

    pub const REG_DMA3CNT_L: u32 = IoRegisters::BASE + IoRegisters::DMA3CNT_L_OFFSET;

    pub const REG_DMA3CNT_H: u32 = IoRegisters::BASE + IoRegisters::DMA3CNT_H_OFFSET;

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

    pub fn tick(&mut self, cycles: u32) {
        self.io.tick(cycles);
    }

    pub fn run_pending_dma(&mut self) -> Option<DmaRunResult> {
        /*
         * Extract the request first, releasing the mutable borrow of
         * IoRegisters before accessing memory through Bus.
         */
        let request = self.io.dma_mut().next_pending_request()?;

        let width_bytes = request.width.bytes();

        let mut source = align_dma_address(request.source, request.width);

        let mut destination = align_dma_address(request.destination, request.width);

        for _ in 0..request.count {
            match request.width {
                DmaTransferWidth::Halfword => {
                    let value = self.read16(source);

                    self.write16(destination, value);
                }

                DmaTransferWidth::Word => {
                    let value = self.read32(source);

                    self.write32(destination, value);
                }
            }

            source = advance_dma_address(source, width_bytes, request.source_control, true);

            destination =
                advance_dma_address(destination, width_bytes, request.destination_control, false);
        }

        let request_interrupt = request.irq_enabled;

        self.io.dma_mut().complete_transfer(DmaTransferCompletion {
            channel: request.channel,
            final_source: source,
            final_destination: destination,
            transferred_units: request.count,
            request_interrupt,
        });

        if request_interrupt {
            self.io
                .request_interrupt(request.channel.interrupt_source());
        }

        /*
         * Placeholder timing:
         *
         * one read plus one write per transfer unit.
         *
         * Wait states and sequential/non-sequential accesses will
         * replace this approximation later.
         */
        let cycles = request.count.saturating_mul(2).max(1);

        Some(DmaRunResult {
            channel: request.channel,
            transferred_units: request.count,
            cycles,
        })
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

fn align_dma_address(address: u32, width: DmaTransferWidth) -> u32 {
    match width {
        DmaTransferWidth::Halfword => address & !1,

        DmaTransferWidth::Word => address & !3,
    }
}

fn advance_dma_address(address: u32, width: u32, control: DmaAddressControl, source: bool) -> u32 {
    match control {
        DmaAddressControl::Increment | DmaAddressControl::IncrementReload => {
            address.wrapping_add(width)
        }

        DmaAddressControl::Decrement => address.wrapping_sub(width),

        DmaAddressControl::Fixed => address,
    }
}

#[cfg(test)]
mod tests {
    use super::{Bus, BusLoadError, DmaChannelIndex, InterruptController, InterruptSource};

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

    #[test]
    fn timer_zero_is_accessible_through_bus_mmio() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_TM0CNT_L, 0xFFF0);

        bus.write16(Bus::REG_TM0CNT_H, 1 << 7);

        bus.tick(5);

        assert_eq!(bus.read16(Bus::REG_TM0CNT_L), 0xFFF5);
    }

    #[test]
    fn timer_overflow_requests_interrupt_through_bus() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        bus.write16(Bus::REG_IME, 1);

        bus.write16(Bus::REG_TM0CNT_L, 0xFFFF);

        bus.write16(Bus::REG_TM0CNT_H, (1 << 7) | (1 << 6));

        assert!(!bus.irq_line());

        bus.tick(1);

        assert!(bus.irq_line());

        assert_eq!(
            bus.read16(Bus::REG_IF) & InterruptSource::Timer0.mask(),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn immediate_dma_copies_halfwords() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 0x1111);

        bus.write16(0x0200_0102, 0x2222);

        bus.write16(0x0200_0104, 0x3333);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 3);

        /*
         * Immediate, halfword, enable.
         */
        bus.write16(Bus::REG_DMA0CNT_H, 1 << 15);

        let result = bus.run_pending_dma().unwrap();

        assert_eq!(result.channel, DmaChannelIndex::Dma0);

        assert_eq!(result.transferred_units, 3);

        assert_eq!(bus.read16(0x0300_0100), 0x1111);

        assert_eq!(bus.read16(0x0300_0102), 0x2222);

        assert_eq!(bus.read16(0x0300_0104), 0x3333);

        /*
         * Enable clears after immediate transfer.
         */
        assert_eq!(bus.read16(Bus::REG_DMA0CNT_H) & (1 << 15), 0);
    }

    #[test]
    fn immediate_dma_copies_words() {
        let mut bus = Bus::new();

        bus.write32(0x0200_0100, 0x1111_1111);

        bus.write32(0x0200_0104, 0x2222_2222);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 2);

        /*
         * 32-bit transfer + enable.
         */
        bus.write16(Bus::REG_DMA0CNT_H, (1 << 10) | (1 << 15));

        bus.run_pending_dma().unwrap();

        assert_eq!(bus.read32(0x0300_0100), 0x1111_1111);

        assert_eq!(bus.read32(0x0300_0104), 0x2222_2222);
    }

    #[test]
    fn dma_fixed_destination_repeatedly_writes_same_address() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 1);
        bus.write16(0x0200_0102, 2);
        bus.write16(0x0200_0104, 3);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 3);

        /*
         * Destination fixed:
         *
         * destination control = 0b10.
         */
        bus.write16(Bus::REG_DMA0CNT_H, (0b10 << 5) | (1 << 15));

        bus.run_pending_dma().unwrap();

        /*
         * Last source value wins.
         */
        assert_eq!(bus.read16(0x0300_0100), 3);
    }

    #[test]
    fn dma_can_decrement_source_and_destination() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 0x1111);
        bus.write16(0x0200_0102, 0x2222);
        bus.write16(0x0200_0104, 0x3333);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0104);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0104);

        bus.write16(Bus::REG_DMA0CNT_L, 3);

        /*
         * Destination decrement = 01
         * Source decrement      = 01
         */
        bus.write16(Bus::REG_DMA0CNT_H, (0b01 << 5) | (0b01 << 7) | (1 << 15));

        bus.run_pending_dma().unwrap();

        assert_eq!(bus.read16(0x0300_0104), 0x3333);

        assert_eq!(bus.read16(0x0300_0102), 0x2222);

        assert_eq!(bus.read16(0x0300_0100), 0x1111);
    }

    #[test]
    fn dma_completion_sets_interrupt_flag() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 0xCAFE);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 1);

        /*
         * IRQ on completion + enable.
         */
        bus.write16(Bus::REG_DMA0CNT_H, (1 << 14) | (1 << 15));

        bus.run_pending_dma().unwrap();

        assert_eq!(
            bus.read16(Bus::REG_IF) & InterruptSource::Dma0.mask(),
            InterruptSource::Dma0.mask()
        );
    }
}
