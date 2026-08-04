use std::error::Error;
use std::fmt;

mod cartridge_save;
mod dma;
mod interrupt;
mod io;
mod keypad;
mod memory;
mod power;
mod ppu;
mod timer;
mod video;
mod waitstate;

pub use self::dma::{
    DMA_CHANNEL_COUNT, DmaAddressControl, DmaChannel, DmaChannelIndex, DmaControl, DmaController,
    DmaStartTiming, DmaTransferCompletion, DmaTransferRequest, DmaTransferWidth,
};

pub use self::interrupt::{InterruptController, InterruptSource};

pub use self::io::IoRegisters;

pub use self::keypad::{Key, KeyControl, Keypad, KeypadInterruptCondition, KeypadUpdateResult};

pub use self::power::{PowerControl, PowerStateRequest};

pub use self::ppu::{DispStat, Ppu, PpuTickResult, VisibleScanlineIter, VisibleScanlineSet};

pub use self::timer::{TIMER_COUNT, Timer, TimerControl, TimerController, TimerIndex};

pub use self::video::{
    AffineBackground, AffineBackgroundControl, DisplayControl, FRAMEBUFFER_PIXEL_COUNT,
    Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH, TextBackground, TextBackgroundControl, Video,
    VideoMode, bgr555_to_rgba8888,
};

pub use self::waitstate::{AccessKind, AccessWidth, MemoryRegion, TimedAccess, WaitControl};

pub use self::cartridge_save::{CartridgeSaveLoadError, CartridgeSaveType};

use self::cartridge_save::CartridgeSave;
use self::memory::{read_u16_le, read_u32_le, write_u16_le, write_u32_le};

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
const EEPROM_LARGE_ROM_BASE: u32 = 0x0DFF_FF00;
const EEPROM_SMALL_ROM_BASE: u32 = 0x0D00_0000;
const EEPROM_SMALL_ROM_LIMIT: usize = 16 * 1024 * 1024;

const SAVE_BASE: u32 = 0x0E00_0000;
const SAVE_WINDOW_SIZE: usize = 0x0001_0000;

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
    cartridge_save: CartridgeSave,
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

    pub const REG_DISPSTAT: u32 = IoRegisters::BASE + IoRegisters::DISPSTAT_OFFSET;

    pub const REG_VCOUNT: u32 = IoRegisters::BASE + IoRegisters::VCOUNT_OFFSET;

    pub const REG_KEYINPUT: u32 = IoRegisters::BASE + IoRegisters::KEYINPUT_OFFSET;

    pub const REG_KEYCNT: u32 = IoRegisters::BASE + IoRegisters::KEYCNT_OFFSET;

    pub const REG_POSTFLG: u32 = IoRegisters::BASE + IoRegisters::POSTFLG_OFFSET;

    pub const REG_HALTCNT: u32 = IoRegisters::BASE + IoRegisters::HALTCNT_OFFSET;

    pub const REG_WAITCNT: u32 = IoRegisters::BASE + IoRegisters::WAITCNT_OFFSET;

    pub const REG_DISPCNT: u32 = IoRegisters::BASE + IoRegisters::DISPCNT_OFFSET;

    pub const REG_BG0CNT: u32 = IoRegisters::BASE + IoRegisters::BG0CNT_OFFSET;
    pub const REG_BG1CNT: u32 = IoRegisters::BASE + IoRegisters::BG1CNT_OFFSET;
    pub const REG_BG2CNT: u32 = IoRegisters::BASE + IoRegisters::BG2CNT_OFFSET;
    pub const REG_BG3CNT: u32 = IoRegisters::BASE + IoRegisters::BG3CNT_OFFSET;

    pub const REG_BG0HOFS: u32 = IoRegisters::BASE + IoRegisters::BG0HOFS_OFFSET;
    pub const REG_BG0VOFS: u32 = IoRegisters::BASE + IoRegisters::BG0VOFS_OFFSET;
    pub const REG_BG1HOFS: u32 = IoRegisters::BASE + IoRegisters::BG1HOFS_OFFSET;
    pub const REG_BG1VOFS: u32 = IoRegisters::BASE + IoRegisters::BG1VOFS_OFFSET;
    pub const REG_BG2HOFS: u32 = IoRegisters::BASE + IoRegisters::BG2HOFS_OFFSET;
    pub const REG_BG2VOFS: u32 = IoRegisters::BASE + IoRegisters::BG2VOFS_OFFSET;
    pub const REG_BG3HOFS: u32 = IoRegisters::BASE + IoRegisters::BG3HOFS_OFFSET;
    pub const REG_BG3VOFS: u32 = IoRegisters::BASE + IoRegisters::BG3VOFS_OFFSET;

    pub const REG_WIN0H: u32 = IoRegisters::BASE + IoRegisters::WIN0H_OFFSET;
    pub const REG_WIN1H: u32 = IoRegisters::BASE + IoRegisters::WIN1H_OFFSET;
    pub const REG_WIN0V: u32 = IoRegisters::BASE + IoRegisters::WIN0V_OFFSET;
    pub const REG_WIN1V: u32 = IoRegisters::BASE + IoRegisters::WIN1V_OFFSET;
    pub const REG_WININ: u32 = IoRegisters::BASE + IoRegisters::WININ_OFFSET;
    pub const REG_WINOUT: u32 = IoRegisters::BASE + IoRegisters::WINOUT_OFFSET;

    pub const REG_BLDCNT: u32 = IoRegisters::BASE + IoRegisters::BLDCNT_OFFSET;
    pub const REG_BLDALPHA: u32 = IoRegisters::BASE + IoRegisters::BLDALPHA_OFFSET;
    pub const REG_BLDY: u32 = IoRegisters::BASE + IoRegisters::BLDY_OFFSET;

    pub const REG_BG2PA: u32 = IoRegisters::BASE + IoRegisters::BG2PA_OFFSET;
    pub const REG_BG2PB: u32 = IoRegisters::BASE + IoRegisters::BG2PB_OFFSET;
    pub const REG_BG2PC: u32 = IoRegisters::BASE + IoRegisters::BG2PC_OFFSET;
    pub const REG_BG2PD: u32 = IoRegisters::BASE + IoRegisters::BG2PD_OFFSET;
    pub const REG_BG2X: u32 = IoRegisters::BASE + IoRegisters::BG2X_L_OFFSET;
    pub const REG_BG2Y: u32 = IoRegisters::BASE + IoRegisters::BG2Y_L_OFFSET;

    pub const REG_BG3PA: u32 = IoRegisters::BASE + IoRegisters::BG3PA_OFFSET;
    pub const REG_BG3PB: u32 = IoRegisters::BASE + IoRegisters::BG3PB_OFFSET;
    pub const REG_BG3PC: u32 = IoRegisters::BASE + IoRegisters::BG3PC_OFFSET;
    pub const REG_BG3PD: u32 = IoRegisters::BASE + IoRegisters::BG3PD_OFFSET;
    pub const REG_BG3X: u32 = IoRegisters::BASE + IoRegisters::BG3X_L_OFFSET;
    pub const REG_BG3Y: u32 = IoRegisters::BASE + IoRegisters::BG3Y_L_OFFSET;

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

            cartridge_save: CartridgeSave::default(),
        }
    }

    pub fn reset(&mut self) {
        self.ewram.fill(0);
        self.iwram.fill(0);
        self.io.reset();
        self.palette.fill(0);
        self.vram.fill(0);
        self.oam.fill(0);
        self.cartridge_save.reset_protocol();

        /*
         * BIOS, cartridge ROM and non-volatile cartridge save are preserved
         * across reset.
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
        self.cartridge_save = CartridgeSave::from_rom(rom);

        Ok(())
    }

    pub const fn cartridge_save_type(&self) -> CartridgeSaveType {
        self.cartridge_save.save_type()
    }

    pub fn cartridge_save_data(&self) -> &[u8] {
        self.cartridge_save.data()
    }

    pub fn load_cartridge_save(&mut self, data: &[u8]) -> Result<(), CartridgeSaveLoadError> {
        self.cartridge_save.load_data(data)
    }

    pub const fn cartridge_save_dirty(&self) -> bool {
        self.cartridge_save.is_dirty()
    }

    pub fn mark_cartridge_save_clean(&mut self) {
        self.cartridge_save.mark_clean();
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

    pub fn set_key(&mut self, key: Key, pressed: bool) {
        self.io.set_key(key, pressed);
    }

    pub fn set_pressed_keys(&mut self, pressed_mask: u16) {
        self.io.set_pressed_keys(pressed_mask);
    }

    pub fn take_power_request(&mut self) -> Option<PowerStateRequest> {
        self.io.take_power_request()
    }

    pub const fn halt_wake_requested(&self) -> bool {
        /*
         * HALT wake-up is based on an enabled pending interrupt source.
         *
         * Keep this distinct from irq_line(), which additionally requires
         * IME.
         */
        self.interrupt_controller().enabled_pending() != 0
    }

    pub const fn wait_control(&self) -> &WaitControl {
        self.io.wait_control()
    }

    pub fn access_cycles(&self, address: u32, width: AccessWidth, kind: AccessKind) -> u32 {
        self.io.wait_control().access_cycles(address, width, kind)
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.io.video().framebuffer()
    }

    pub const fn frame_ready(&self) -> bool {
        self.io.video().frame_ready()
    }

    pub fn take_frame_ready(&mut self) -> bool {
        self.io.video_mut().take_frame_ready()
    }

    pub const fn frame_number(&self) -> u64 {
        self.io.video().frame_number()
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
                let offset = mirror_offset(address, SAVE_BASE, SAVE_WINDOW_SIZE);

                self.cartridge_save.read8(offset)
            }

            _ => 0,
        }
    }

    pub fn read8_timed(&self, address: u32, kind: AccessKind) -> TimedAccess<u8> {
        let cycles = self.access_cycles(address, AccessWidth::Byte, kind);

        TimedAccess::new(self.read8(address), cycles)
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
                /*
                 * Palette RAM is connected through a 16-bit bus. Hardware
                 * replicates an 8-bit write across the addressed halfword.
                 */
                let offset = mirror_offset(address, PALETTE_BASE, PALETTE_SIZE) & !1;

                self.palette[offset] = value;
                self.palette[offset + 1] = value;
            }

            0x0600_0000..=0x06FF_FFFF => {
                /*
                 * VRAM has the same byte-write replication behavior as
                 * Palette RAM.
                 */
                let offset = vram_offset(address) & !1;

                self.vram[offset] = value;
                self.vram[offset + 1] = value;
            }

            /*
             * OAM ignores byte writes.
             */
            0x0700_0000..=0x07FF_FFFF => {}

            /*
             * Game Pak ROM is read-only.
             */
            ROM_BASE..=ROM_END => {}

            0x0E00_0000..=0x0EFF_FFFF => {
                let offset = mirror_offset(address, SAVE_BASE, SAVE_WINDOW_SIZE);

                self.cartridge_save.write8(offset, value);
            }

            _ => {}
        }
    }

    pub fn write8_timed(&mut self, address: u32, value: u8, kind: AccessKind) -> u32 {
        let cycles = self.access_cycles(address, AccessWidth::Byte, kind);

        self.write8(address, value);

        cycles
    }

    pub fn read16(&self, address: u32) -> u16 {
        let aligned = address & !1;

        if self.is_eeprom_access(aligned) {
            /* Direct CPU reads do not clock an EEPROM DMA transaction. */
            return 1;
        }

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            return self.io.read16(offset);
        }

        let direct = match aligned {
            BIOS_BASE..=0x0000_3FFF => Some(read_u16_le(
                self.bios.as_slice(),
                (aligned - BIOS_BASE) as usize,
            )),
            0x0200_0000..=0x02FF_FFFF => Some(read_u16_le(
                self.ewram.as_slice(),
                mirror_offset(aligned, EWRAM_BASE, EWRAM_SIZE),
            )),
            0x0300_0000..=0x03FF_FFFF => Some(read_u16_le(
                self.iwram.as_slice(),
                mirror_offset(aligned, IWRAM_BASE, IWRAM_SIZE),
            )),
            0x0500_0000..=0x05FF_FFFF => Some(read_u16_le(
                self.palette.as_slice(),
                mirror_offset(aligned, PALETTE_BASE, PALETTE_SIZE),
            )),
            0x0600_0000..=0x06FF_FFFF => {
                Some(read_u16_le(self.vram.as_slice(), vram_offset(aligned)))
            }
            0x0700_0000..=0x07FF_FFFF => Some(read_u16_le(
                self.oam.as_slice(),
                mirror_offset(aligned, OAM_BASE, OAM_SIZE),
            )),
            ROM_BASE..=ROM_END => Some(read_u16_le(
                self.rom.as_slice(),
                ((aligned - ROM_BASE) & 0x01FF_FFFF) as usize,
            )),
            _ => None,
        };

        if let Some(value) = direct {
            return value;
        }

        let low = self.read8(aligned);
        let high = self.read8(aligned.wrapping_add(1));

        u16::from_le_bytes([low, high])
    }

    pub fn read16_timed(&self, address: u32, kind: AccessKind) -> TimedAccess<u16> {
        let cycles = self.access_cycles(address, AccessWidth::Halfword, kind);

        TimedAccess::new(self.read16(address), cycles)
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        let aligned = address & !1;

        if self.is_eeprom_access(aligned) {
            self.cartridge_save.write_eeprom_bit(value & 1 != 0);
            return;
        }

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            self.io.write16(offset, value);

            return;
        }

        let [low, high] = value.to_le_bytes();

        match aligned {
            0x0200_0000..=0x02FF_FFFF => {
                let offset = mirror_offset(aligned, EWRAM_BASE, EWRAM_SIZE);
                write_u16_le(self.ewram.as_mut_slice(), offset, value);
                return;
            }

            0x0300_0000..=0x03FF_FFFF => {
                let offset = mirror_offset(aligned, IWRAM_BASE, IWRAM_SIZE);
                write_u16_le(self.iwram.as_mut_slice(), offset, value);
                return;
            }

            0x0500_0000..=0x05FF_FFFF => {
                let offset = mirror_offset(aligned, PALETTE_BASE, PALETTE_SIZE);

                self.palette[offset] = low;
                self.palette[offset + 1] = high;

                return;
            }

            0x0600_0000..=0x06FF_FFFF => {
                let offset = vram_offset(aligned);

                self.vram[offset] = low;
                self.vram[offset + 1] = high;

                return;
            }

            0x0700_0000..=0x07FF_FFFF => {
                let offset = mirror_offset(aligned, OAM_BASE, OAM_SIZE);

                self.oam[offset] = low;
                self.oam[offset + 1] = high;

                return;
            }

            _ => {}
        }

        self.write8(aligned, low);

        self.write8(aligned.wrapping_add(1), high);
    }

    pub fn write16_timed(&mut self, address: u32, value: u16, kind: AccessKind) -> u32 {
        let cycles = self.access_cycles(address, AccessWidth::Halfword, kind);

        self.write16(address, value);

        cycles
    }

    pub fn read32(&self, address: u32) -> u32 {
        let aligned = address & !3;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            return self.io.read32(offset);
        }

        let direct = match aligned {
            BIOS_BASE..=0x0000_3FFF => Some(read_u32_le(
                self.bios.as_slice(),
                (aligned - BIOS_BASE) as usize,
            )),
            0x0200_0000..=0x02FF_FFFF => Some(read_u32_le(
                self.ewram.as_slice(),
                mirror_offset(aligned, EWRAM_BASE, EWRAM_SIZE),
            )),
            0x0300_0000..=0x03FF_FFFF => Some(read_u32_le(
                self.iwram.as_slice(),
                mirror_offset(aligned, IWRAM_BASE, IWRAM_SIZE),
            )),
            0x0500_0000..=0x05FF_FFFF => Some(read_u32_le(
                self.palette.as_slice(),
                mirror_offset(aligned, PALETTE_BASE, PALETTE_SIZE),
            )),
            0x0600_0000..=0x06FF_FFFF => {
                Some(read_u32_le(self.vram.as_slice(), vram_offset(aligned)))
            }
            0x0700_0000..=0x07FF_FFFF => Some(read_u32_le(
                self.oam.as_slice(),
                mirror_offset(aligned, OAM_BASE, OAM_SIZE),
            )),
            ROM_BASE..=ROM_END => Some(read_u32_le(
                self.rom.as_slice(),
                ((aligned - ROM_BASE) & 0x01FF_FFFF) as usize,
            )),
            _ => None,
        };

        if let Some(value) = direct {
            return value;
        }

        let b0 = self.read8(aligned);

        let b1 = self.read8(aligned.wrapping_add(1));

        let b2 = self.read8(aligned.wrapping_add(2));

        let b3 = self.read8(aligned.wrapping_add(3));

        u32::from_le_bytes([b0, b1, b2, b3])
    }

    pub fn read32_timed(&self, address: u32, kind: AccessKind) -> TimedAccess<u32> {
        let cycles = self.access_cycles(address, AccessWidth::Word, kind);

        TimedAccess::new(self.read32(address), cycles)
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        let aligned = address & !3;

        if IoRegisters::contains_address(aligned) {
            let offset = IoRegisters::address_to_offset(aligned);

            self.io.write32(offset, value);

            return;
        }

        match aligned {
            0x0200_0000..=0x02FF_FFFF => {
                let offset = mirror_offset(aligned, EWRAM_BASE, EWRAM_SIZE);
                write_u32_le(self.ewram.as_mut_slice(), offset, value);
                return;
            }
            0x0300_0000..=0x03FF_FFFF => {
                let offset = mirror_offset(aligned, IWRAM_BASE, IWRAM_SIZE);
                write_u32_le(self.iwram.as_mut_slice(), offset, value);
                return;
            }
            0x0500_0000..=0x05FF_FFFF => {
                let offset = mirror_offset(aligned, PALETTE_BASE, PALETTE_SIZE);
                write_u32_le(self.palette.as_mut_slice(), offset, value);
                return;
            }
            0x0600_0000..=0x06FF_FFFF => {
                let offset = vram_offset(aligned);
                write_u32_le(self.vram.as_mut_slice(), offset, value);
                return;
            }
            0x0700_0000..=0x07FF_FFFF => {
                let offset = mirror_offset(aligned, OAM_BASE, OAM_SIZE);
                write_u32_le(self.oam.as_mut_slice(), offset, value);
                return;
            }
            _ => {}
        }

        let bytes = value.to_le_bytes();

        for (index, byte) in bytes.into_iter().enumerate() {
            self.write8(aligned.wrapping_add(index as u32), byte);
        }
    }

    pub fn write32_timed(&mut self, address: u32, value: u32, kind: AccessKind) -> u32 {
        let cycles = self.access_cycles(address, AccessWidth::Word, kind);

        self.write32(address, value);

        cycles
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

    fn is_eeprom_access(&self, address: u32) -> bool {
        if !self.cartridge_save.is_eeprom() {
            return false;
        }

        if self.rom.len() <= EEPROM_SMALL_ROM_LIMIT {
            (EEPROM_SMALL_ROM_BASE..=ROM_END).contains(&address)
        } else {
            (EEPROM_LARGE_ROM_BASE..=ROM_END).contains(&address)
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        let result = self.io.tick(cycles);

        /*
         * Render after PPU timing has reported which visible lines have
         * completed.
         *
         * The immutable VRAM borrow and mutable video borrow target
         * separate Bus fields, so Rust permits this split borrow.
         */
        {
            let vram = self.vram.as_slice();
            let palette = self.palette.as_slice();
            let oam = self.oam.as_slice();
            let video = self.io.video_mut();

            /*
             * VCOUNT wrapped from 227 to 0. Reload affine reference points
             * before rendering the first visible scanline of the new frame.
             */
            if result.new_frames != 0 {
                video.begin_frame();
            }

            for line in result.completed_visible_lines.iter() {
                video.render_scanline(line, vram, palette, oam);
            }

            /*
             * VBlank begins only after all 160 visible lines have entered
             * HBlank and therefore been rendered.
             */
            if result.vblank_starts != 0 {
                video.mark_frame_ready();
            }
        }
    }

    pub fn run_pending_dma(&mut self) -> Option<DmaRunResult> {
        /*
         * Take a snapshot of the pending DMA request first.
         *
         * This releases the mutable borrow of IoRegisters before DMA
         * accesses memory through Bus.
         */
        let request = self.io.dma_mut().next_pending_request()?;

        let width_bytes = request.width.bytes();

        let mut source = align_dma_address(request.source, request.width);

        let mut destination = align_dma_address(request.destination, request.width);

        let mut cycles = 0u32;

        let eeprom_dma =
            request.channel == DmaChannelIndex::Dma3 && request.width == DmaTransferWidth::Halfword;
        let eeprom_source = eeprom_dma && self.is_eeprom_access(source);
        let eeprom_destination = eeprom_dma && self.is_eeprom_access(destination);

        if eeprom_source || eeprom_destination {
            self.cartridge_save
                .begin_eeprom_dma(request.count, eeprom_destination);
        }

        for transfer_index in 0..request.count {
            /*
             * First transfer unit is non-sequential.
             * Following units continue sequentially.
             *
             * Source and destination use separate variables so their
             * sequencing can be modeled independently later.
             */
            let source_kind = if transfer_index == 0 {
                AccessKind::NonSequential
            } else {
                AccessKind::Sequential
            };

            let destination_kind = if transfer_index == 0 {
                AccessKind::NonSequential
            } else {
                AccessKind::Sequential
            };

            match request.width {
                DmaTransferWidth::Halfword => {
                    let read = if eeprom_source {
                        TimedAccess::new(
                            self.cartridge_save.read_eeprom_bit() as u16,
                            self.access_cycles(source, AccessWidth::Halfword, source_kind),
                        )
                    } else {
                        self.read16_timed(source, source_kind)
                    };

                    let write_cycles =
                        self.write16_timed(destination, read.value, destination_kind);

                    cycles = cycles
                        .saturating_add(read.cycles)
                        .saturating_add(write_cycles);
                }

                DmaTransferWidth::Word => {
                    let read = self.read32_timed(source, source_kind);

                    let write_cycles =
                        self.write32_timed(destination, read.value, destination_kind);

                    cycles = cycles
                        .saturating_add(read.cycles)
                        .saturating_add(write_cycles);
                }
            }

            source = advance_dma_address(source, width_bytes, request.source_control);

            destination =
                advance_dma_address(destination, width_bytes, request.destination_control);
        }

        self.io.dma_mut().complete_transfer(DmaTransferCompletion {
            channel: request.channel,
            final_source: source,
            final_destination: destination,
            transferred_units: request.count,
        });

        if request.irq_enabled {
            self.io
                .request_interrupt(request.channel.interrupt_source());
        }

        Some(DmaRunResult {
            channel: request.channel,
            transferred_units: request.count,

            /*
             * A zero-cycle DMA scheduling unit would be dangerous for the
             * outer scheduler. Count is normally never zero after DMA
             * latching, but keep this defensive guard.
             */
            cycles: cycles.max(1),
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

fn advance_dma_address(address: u32, width: u32, control: DmaAddressControl) -> u32 {
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
    use super::{
        AccessKind, Bus, BusLoadError, CartridgeSaveType, DmaChannelIndex, InterruptController,
        InterruptSource, Ppu, SCREEN_WIDTH,
    };

    fn append_serial_bits(bits: &mut Vec<bool>, value: usize, count: usize) {
        for shift in (0..count).rev() {
            bits.push(value & (1 << shift) != 0);
        }
    }

    fn write_dma_bit_buffer(bus: &mut Bus, address: u32, bits: &[bool]) {
        for (index, bit) in bits.iter().copied().enumerate() {
            bus.write16(address + index as u32 * 2, bit as u16);
        }
    }

    fn run_dma3_halfwords(bus: &mut Bus, source: u32, destination: u32, count: u16) {
        bus.write32(Bus::REG_DMA3SAD, source);
        bus.write32(Bus::REG_DMA3DAD, destination);
        bus.write16(Bus::REG_DMA3CNT_L, count);
        bus.write16(Bus::REG_DMA3CNT_H, 1 << 15);

        let result = bus.run_pending_dma().unwrap();
        assert_eq!(result.channel, DmaChannelIndex::Dma3);
        assert_eq!(result.transferred_units, count as u32);
    }

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
    fn palette_byte_write_is_replicated_across_halfword() {
        let mut bus = Bus::new();

        bus.write8(0x0500_0001, 0xAB);

        assert_eq!(bus.read16(0x0500_0000), 0xABAB);
    }

    #[test]
    fn vram_byte_write_is_replicated_across_halfword() {
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
    fn special_video_memory_halfword_writes_preserve_both_bytes() {
        let mut bus = Bus::new();

        bus.write16(0x0500_0000, 0x1234);
        bus.write16(0x0600_0000, 0x5678);
        bus.write16(0x0700_0000, 0x9ABC);

        assert_eq!(bus.read16(0x0500_0000), 0x1234);
        assert_eq!(bus.read16(0x0600_0000), 0x5678);
        assert_eq!(bus.read16(0x0700_0000), 0x9ABC);
    }

    #[test]
    fn flash_1m_rom_exposes_id_and_preserves_save_across_reset() {
        const SAVE: u32 = 0x0E00_0000;

        let mut bus = Bus::new();

        bus.load_rom(b"FLASH1M_V103").unwrap();

        bus.write8(SAVE + 0x5555, 0xAA);
        bus.write8(SAVE + 0x2AAA, 0x55);
        bus.write8(SAVE + 0x5555, 0x90);

        assert_eq!(bus.read8(SAVE), 0x62);
        assert_eq!(bus.read8(SAVE + 1), 0x13);

        bus.write8(SAVE + 0x5555, 0xAA);
        bus.write8(SAVE + 0x2AAA, 0x55);
        bus.write8(SAVE + 0x5555, 0xF0);

        bus.write8(SAVE + 0x5555, 0xAA);
        bus.write8(SAVE + 0x2AAA, 0x55);
        bus.write8(SAVE + 0x5555, 0xA0);
        bus.write8(SAVE + 0x1234, 0x5A);

        bus.write8(SAVE + 0x5555, 0xAA);
        bus.write8(SAVE + 0x2AAA, 0x55);
        bus.write8(SAVE + 0x5555, 0x90);

        bus.reset();

        assert_eq!(bus.read8(SAVE), 0xFF);
        assert_eq!(bus.read8(SAVE + 0x1234), 0x5A);
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
    fn dma3_round_trips_eeprom_serial_data() {
        const BUFFER: u32 = 0x0200_0100;
        const READBACK: u32 = 0x0300_0100;
        const EEPROM: u32 = 0x0D00_0000;

        let mut bus = Bus::new();
        bus.load_rom(b"header EEPROM_V124 trailer").unwrap();

        let block = 0x2A;
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let mut write_command = vec![true, false];
        append_serial_bits(&mut write_command, block, 6);

        for byte in data {
            append_serial_bits(&mut write_command, byte as usize, 8);
        }

        write_command.push(false);
        assert_eq!(write_command.len(), 73);
        write_dma_bit_buffer(&mut bus, BUFFER, &write_command);
        run_dma3_halfwords(&mut bus, BUFFER, EEPROM, 73);

        assert_eq!(bus.cartridge_save_type(), CartridgeSaveType::Eeprom512);
        assert!(bus.cartridge_save_dirty());

        let mut read_command = vec![true, true];
        append_serial_bits(&mut read_command, block, 6);
        read_command.push(false);
        write_dma_bit_buffer(&mut bus, BUFFER, &read_command);
        run_dma3_halfwords(&mut bus, BUFFER, EEPROM, 9);
        run_dma3_halfwords(&mut bus, EEPROM, READBACK, 68);

        assert!((0..4).all(|index| bus.read16(READBACK + index * 2) & 1 == 0));

        let read_bits: Vec<bool> = (4..68)
            .map(|index| bus.read16(READBACK + index * 2) & 1 != 0)
            .collect();
        let decoded: Vec<u8> = read_bits
            .chunks_exact(8)
            .map(|bits| {
                bits.iter()
                    .fold(0u8, |value, &bit| (value << 1) | bit as u8)
            })
            .collect();

        assert_eq!(decoded, data);
    }

    #[test]
    fn eeprom_address_window_depends_on_physical_rom_size() {
        const EEPROM_FULL_WINDOW: u32 = 0x0D00_0000;
        const EEPROM_LARGE_ROM_WINDOW: u32 = 0x0DFF_FF00;

        let mut small_bus = Bus::new();
        small_bus.load_rom(b"EEPROM_V124").unwrap();
        assert!(small_bus.is_eeprom_access(EEPROM_FULL_WINDOW));

        let mut large_rom = vec![0; 16 * 1024 * 1024 + 1];
        large_rom[..11].copy_from_slice(b"EEPROM_V124");
        let mut large_bus = Bus::new();
        large_bus.load_rom(&large_rom).unwrap();

        assert!(!large_bus.is_eeprom_access(EEPROM_FULL_WINDOW));
        assert!(large_bus.is_eeprom_access(EEPROM_LARGE_ROM_WINDOW));
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

    #[test]
    fn vblank_repeat_dma_runs_once_per_vblank_event() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 0x1111);

        bus.write16(0x0200_0102, 0x2222);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 1);

        /*
         * Repeat
         * VBlank timing
         * Enable
         */
        bus.write16(Bus::REG_DMA0CNT_H, (1 << 9) | (0b01 << 12) | (1 << 15));

        assert!(bus.run_pending_dma().is_none());

        /*
         * Reach VBlank.
         */
        bus.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        bus.run_pending_dma().unwrap();

        assert_eq!(bus.read16(0x0300_0100), 0x1111,);

        /*
         * Repeat DMA stays enabled.
         */
        assert_ne!(bus.read16(Bus::REG_DMA0CNT_H) & (1 << 15), 0,);
    }

    #[test]
    fn ewram_word_read_costs_six_cycles() {
        let bus = Bus::new();

        let access = bus.read32_timed(0x0200_0000, AccessKind::NonSequential);

        assert_eq!(access.cycles, 6);
    }

    #[test]
    fn iwram_word_read_costs_one_cycle() {
        let bus = Bus::new();

        let access = bus.read32_timed(0x0300_0000, AccessKind::NonSequential);

        assert_eq!(access.cycles, 1);
    }

    #[test]
    fn waitcnt_changes_game_pak_timing() {
        let mut bus = Bus::new();

        assert_eq!(
            bus.read16_timed(0x0800_0000, AccessKind::NonSequential,)
                .cycles,
            4,
        );

        /*
         * WS0 first = 2 cycles.
         */
        bus.write16(Bus::REG_WAITCNT, 0b10 << 2);

        assert_eq!(
            bus.read16_timed(0x0800_0000, AccessKind::NonSequential,)
                .cycles,
            2,
        );
    }

    #[test]
    fn sequential_game_pak_access_uses_second_timing() {
        let mut bus = Bus::new();

        /*
         * WS0 first = 3
         * WS0 second = 1
         */
        bus.write16(Bus::REG_WAITCNT, (0b01 << 2) | (1 << 4));

        assert_eq!(
            bus.read16_timed(0x0800_0000, AccessKind::NonSequential,)
                .cycles,
            3,
        );

        assert_eq!(
            bus.read16_timed(0x0800_0002, AccessKind::Sequential,)
                .cycles,
            1,
        );
    }

    #[test]
    fn dma_uses_memory_access_timing() {
        let mut bus = Bus::new();

        bus.write16(0x0200_0100, 0xCAFE);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0300_0100);

        bus.write16(Bus::REG_DMA0CNT_L, 1);

        bus.write16(Bus::REG_DMA0CNT_H, 1 << 15);

        let result = bus.run_pending_dma().unwrap();

        /*
         * EWRAM halfword read = 3 cycles.
         * IWRAM halfword write = 1 cycle.
         */
        assert_eq!(result.cycles, 4);

        assert_eq!(bus.read16(0x0300_0100), 0xCAFE,);
    }

    #[test]
    fn mode3_pixel_is_rendered_when_hblank_begins() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_DISPCNT, 3);

        /*
         * Pixel 0,0 = red.
         */
        bus.write16(0x0600_0000, 0x001F);

        /*
         * Pixel is not rendered before the line completes.
         */
        assert_ne!(bus.framebuffer()[0], 0xFFFF_0000,);

        bus.tick(Ppu::HDRAW_CYCLES as u32);

        assert_eq!(bus.framebuffer()[0], 0xFFFF_0000,);
    }

    #[test]
    fn mode3_renders_pixel_at_correct_coordinates() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_DISPCNT, 3);

        let x = 25usize;
        let y = 10usize;

        let vram_address = 0x0600_0000 + ((y * SCREEN_WIDTH + x) * 2) as u32;

        bus.write16(vram_address, 0x03E0);

        bus.tick(Ppu::CYCLES_PER_LINE as u32 * y as u32 + Ppu::HDRAW_CYCLES as u32);

        assert_eq!(bus.framebuffer()[y * SCREEN_WIDTH + x], 0xFF00_FF00,);
    }

    #[test]
    fn mode3_frame_becomes_ready_at_vblank() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_DISPCNT, 3);

        assert!(!bus.frame_ready());

        bus.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        assert!(bus.frame_ready());
        assert_eq!(bus.frame_number(), 1);

        assert!(bus.take_frame_ready());
        assert!(!bus.frame_ready());
        assert!(!bus.take_frame_ready());
    }

    #[test]
    fn forced_blank_renders_white() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_DISPCNT, 3 | (1 << 7));

        bus.write16(0x0600_0000, 0x001F);

        bus.tick(Ppu::HDRAW_CYCLES as u32);

        assert_eq!(bus.framebuffer()[0], 0xFFFF_FFFF,);
    }

    #[test]
    fn dma_to_vram_is_visible_in_mode3_framebuffer() {
        let mut bus = Bus::new();

        bus.write16(Bus::REG_DISPCNT, 3);

        /*
         * Source contains one red BGR555 pixel.
         */
        bus.write16(0x0200_0100, 0x001F);

        bus.write32(Bus::REG_DMA0SAD, 0x0200_0100);

        bus.write32(Bus::REG_DMA0DAD, 0x0600_0000);

        bus.write16(Bus::REG_DMA0CNT_L, 1);

        bus.write16(Bus::REG_DMA0CNT_H, 1 << 15);

        bus.run_pending_dma().unwrap();

        bus.tick(Ppu::HDRAW_CYCLES as u32);

        assert_eq!(bus.framebuffer()[0], 0xFFFF_0000,);
    }
}
