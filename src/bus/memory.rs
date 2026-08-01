pub const BIOS_SIZE: usize = 16 * 1024;
pub const EWRAM_SIZE: usize = 256 * 1024;
pub const IWRAM_SIZE: usize = 32 * 1024;
pub const IO_SIZE: usize = 1024;
pub const PALETTE_RAM_SIZE: usize = 1024;
pub const VRAM_SIZE: usize = 96 * 1024;
pub const OAM_SIZE: usize = 1024;
pub const GAME_PAK_ROM_MAX_SIZE: usize = 32 * 1024 * 1024;
pub const SRAM_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub struct Memory {
    pub(crate) bios: Box<[u8; BIOS_SIZE]>,
    pub(crate) ewram: Box<[u8; EWRAM_SIZE]>,
    pub(crate) iwram: Box<[u8; IWRAM_SIZE]>,
    pub(crate) io: Box<[u8; IO_SIZE]>,
    pub(crate) palette_ram: Box<[u8; PALETTE_RAM_SIZE]>,
    pub(crate) vram: Box<[u8; VRAM_SIZE]>,
    pub(crate) oam: Box<[u8; OAM_SIZE]>,
    /// Game Pak ROM size depends on cartridge
    pub(crate) game_pak_rom: Box<[u8]>,
    pub(crate) sram: Box<[u8; SRAM_SIZE]>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            bios: Box::new([0; BIOS_SIZE]),
            ewram: Box::new([0; EWRAM_SIZE]),
            iwram: Box::new([0; IWRAM_SIZE]),
            io: Box::new([0; IO_SIZE]),
            palette_ram: Box::new([0; PALETTE_RAM_SIZE]),
            vram: Box::new([0; VRAM_SIZE]),
            oam: Box::new([0; OAM_SIZE]),
            game_pak_rom: Vec::new().into_boxed_slice(),
            sram: Box::new([0; SRAM_SIZE]),
        }
    }

    pub fn load_bios(&mut self, data: &[u8]) -> Result<(), MemoryLoadError> {
        if data.len() != BIOS_SIZE {
            return Err(MemoryLoadError::InvalidBiosSize {
                expected: BIOS_SIZE,
                actual: data.len(),
            });
        }

        self.bios.copy_from_slice(data);

        Ok(())
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), MemoryLoadError> {
        if data.len() > GAME_PAK_ROM_MAX_SIZE {
            return Err(MemoryLoadError::RomTooLarge {
                maximum: GAME_PAK_ROM_MAX_SIZE,
                actual: data.len(),
            });
        }

        self.game_pak_rom = data.to_vec().into_boxed_slice();

        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryLoadError {
    InvalidBiosSize { expected: usize, actual: usize },
    RomTooLarge { maximum: usize, actual: usize },
}

impl std::fmt::Display for MemoryLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBiosSize { expected, actual } => {
                write!(
                    formatter,
                    "invalid BIOS size: expected {expected} bytes, received {actual} bytes"
                )
            }
            Self::RomTooLarge { maximum, actual } => {
                write!(
                    formatter,
                    "ROM is too large: maximum {maximum} bytes, received {actual} bytes"
                )
            }
        }
    }
}

impl std::error::Error for MemoryLoadError {}
