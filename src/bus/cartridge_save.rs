const SRAM_SIZE: usize = 64 * 1024;
const FLASH_1M_SIZE: usize = 128 * 1024;
const FLASH_BANK_SIZE: usize = 64 * 1024;
const FLASH_SECTOR_SIZE: usize = 4 * 1024;

const FLASH_MAKER_ID: u8 = 0x62;
const FLASH_DEVICE_ID: u8 = 0x13;

#[derive(Debug, Clone)]
pub enum CartridgeSave {
    Sram(Box<[u8; SRAM_SIZE]>),
    Flash1M(Flash1M),
}

impl CartridgeSave {
    pub fn from_rom(rom: &[u8]) -> Self {
        if contains_signature(rom, b"FLASH1M_V") {
            Self::Flash1M(Flash1M::new())
        } else {
            Self::Sram(Box::new([0; SRAM_SIZE]))
        }
    }

    pub fn read8(&self, offset: usize) -> u8 {
        match self {
            Self::Sram(storage) => storage[offset % SRAM_SIZE],
            Self::Flash1M(flash) => flash.read8(offset),
        }
    }

    pub fn write8(&mut self, offset: usize, value: u8) {
        match self {
            Self::Sram(storage) => storage[offset % SRAM_SIZE] = value,
            Self::Flash1M(flash) => flash.write8(offset, value),
        }
    }

    pub fn reset_protocol(&mut self) {
        if let Self::Flash1M(flash) = self {
            flash.reset_protocol();
        }
    }
}

impl Default for CartridgeSave {
    fn default() -> Self {
        Self::Sram(Box::new([0; SRAM_SIZE]))
    }
}

fn contains_signature(bytes: &[u8], signature: &[u8]) -> bool {
    bytes
        .windows(signature.len())
        .any(|window| window == signature)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashCommandState {
    Ready,
    Unlock1,
    Unlock2,
    Program,
    SelectBank,
    EraseUnlock1,
    EraseUnlock2,
    EraseCommand,
}

#[derive(Debug, Clone)]
pub struct Flash1M {
    storage: Box<[u8; FLASH_1M_SIZE]>,
    bank: usize,
    id_mode: bool,
    command_state: FlashCommandState,
}

impl Flash1M {
    pub fn new() -> Self {
        Self {
            storage: Box::new([0xFF; FLASH_1M_SIZE]),
            bank: 0,
            id_mode: false,
            command_state: FlashCommandState::Ready,
        }
    }

    pub fn read8(&self, offset: usize) -> u8 {
        let offset = offset % FLASH_BANK_SIZE;

        if self.id_mode {
            return match offset {
                0 => FLASH_MAKER_ID,
                1 => FLASH_DEVICE_ID,
                _ => 0xFF,
            };
        }

        self.storage[self.bank * FLASH_BANK_SIZE + offset]
    }

    pub fn write8(&mut self, offset: usize, value: u8) {
        let offset = offset % FLASH_BANK_SIZE;

        self.command_state = match self.command_state {
            FlashCommandState::Ready => {
                if offset == 0x5555 && value == 0xAA {
                    FlashCommandState::Unlock1
                } else {
                    FlashCommandState::Ready
                }
            }
            FlashCommandState::Unlock1 => {
                if offset == 0x2AAA && value == 0x55 {
                    FlashCommandState::Unlock2
                } else {
                    FlashCommandState::Ready
                }
            }
            FlashCommandState::Unlock2 => {
                if offset != 0x5555 {
                    FlashCommandState::Ready
                } else {
                    match value {
                        0x90 => {
                            self.id_mode = true;
                            FlashCommandState::Ready
                        }
                        0xF0 => {
                            self.id_mode = false;
                            FlashCommandState::Ready
                        }
                        0xA0 => FlashCommandState::Program,
                        0xB0 => FlashCommandState::SelectBank,
                        0x80 => FlashCommandState::EraseUnlock1,
                        _ => FlashCommandState::Ready,
                    }
                }
            }
            FlashCommandState::Program => {
                let index = self.bank * FLASH_BANK_SIZE + offset;

                /* Flash programming can only clear bits. */
                self.storage[index] &= value;

                FlashCommandState::Ready
            }
            FlashCommandState::SelectBank => {
                if offset == 0 {
                    self.bank = (value & 1) as usize;
                }

                FlashCommandState::Ready
            }
            FlashCommandState::EraseUnlock1 => {
                if offset == 0x5555 && value == 0xAA {
                    FlashCommandState::EraseUnlock2
                } else {
                    FlashCommandState::Ready
                }
            }
            FlashCommandState::EraseUnlock2 => {
                if offset == 0x2AAA && value == 0x55 {
                    FlashCommandState::EraseCommand
                } else {
                    FlashCommandState::Ready
                }
            }
            FlashCommandState::EraseCommand => {
                if offset == 0x5555 && value == 0x10 {
                    self.storage.fill(0xFF);
                } else if value == 0x30 {
                    let bank_start = self.bank * FLASH_BANK_SIZE;
                    let sector_start =
                        bank_start + (offset / FLASH_SECTOR_SIZE) * FLASH_SECTOR_SIZE;

                    self.storage[sector_start..sector_start + FLASH_SECTOR_SIZE].fill(0xFF);
                }

                FlashCommandState::Ready
            }
        };
    }

    pub fn reset_protocol(&mut self) {
        self.bank = 0;
        self.id_mode = false;
        self.command_state = FlashCommandState::Ready;
    }
}

impl Default for Flash1M {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{CartridgeSave, FLASH_DEVICE_ID, FLASH_MAKER_ID, Flash1M};

    fn command(flash: &mut Flash1M, value: u8) {
        flash.write8(0x5555, 0xAA);
        flash.write8(0x2AAA, 0x55);
        flash.write8(0x5555, value);
    }

    fn program(flash: &mut Flash1M, offset: usize, value: u8) {
        command(flash, 0xA0);
        flash.write8(offset, value);
    }

    #[test]
    fn flash_1m_signature_selects_flash_backend() {
        let save = CartridgeSave::from_rom(b"header FLASH1M_V103 trailer");

        assert!(matches!(save, CartridgeSave::Flash1M(_)));
    }

    #[test]
    fn unknown_save_signature_keeps_sram_backend() {
        let save = CartridgeSave::from_rom(b"no known save type");

        assert!(matches!(save, CartridgeSave::Sram(_)));
    }

    #[test]
    fn id_mode_reports_supported_one_megabit_flash() {
        let mut flash = Flash1M::new();

        command(&mut flash, 0x90);

        assert_eq!(flash.read8(0), FLASH_MAKER_ID);
        assert_eq!(flash.read8(1), FLASH_DEVICE_ID);

        command(&mut flash, 0xF0);

        assert_eq!(flash.read8(0), 0xFF);
    }

    #[test]
    fn programming_and_sector_erase_follow_flash_semantics() {
        let mut flash = Flash1M::new();

        program(&mut flash, 0x2345, 0x5A);
        program(&mut flash, 0x2345, 0xF0);

        assert_eq!(flash.read8(0x2345), 0x50);

        command(&mut flash, 0x80);
        flash.write8(0x5555, 0xAA);
        flash.write8(0x2AAA, 0x55);
        flash.write8(0x2345, 0x30);

        assert_eq!(flash.read8(0x2345), 0xFF);
    }

    #[test]
    fn bank_select_exposes_independent_64k_halves() {
        let mut flash = Flash1M::new();

        program(&mut flash, 0x1234, 0x11);

        command(&mut flash, 0xB0);
        flash.write8(0, 1);
        program(&mut flash, 0x1234, 0x22);

        assert_eq!(flash.read8(0x1234), 0x22);

        command(&mut flash, 0xB0);
        flash.write8(0, 0);

        assert_eq!(flash.read8(0x1234), 0x11);
    }
}
