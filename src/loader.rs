use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::bus::{BIOS_SIZE, GAME_PAK_ROM_MAX_SIZE};

const GBA_ROM_HEADER_MIN_SIZE: usize = 0xC0;

const TITLE_START: usize = 0xA0;
const TITLE_END: usize = 0xAC;

const GAME_CODE_START: usize = 0xAC;
const GAME_CODE_END: usize = 0xB0;

const MAKER_CODE_START: usize = 0xB0;
const MAKER_CODE_END: usize = 0xB2;

const FIXED_VALUE_OFFSET: usize = 0xB2;
const SOFTWARE_VERSION_OFFSET: usize = 0xBC;
const HEADER_CHECKSUM_OFFSET: usize = 0xBD;

const EXPECTED_FIXED_VALUE: u8 = 0x96;

#[derive(Debug)]
pub struct BiosImage {
    bytes: Vec<u8>,
}

impl BiosImage {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug)]
pub struct RomImage {
    bytes: Vec<u8>,
    header: RomHeader,
}

impl RomImage {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomHeader {
    title: String,
    game_code: String,
    maker_code: String,
    software_version: u8,
    header_checksum: u8,
}

impl RomHeader {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn game_code(&self) -> &str {
        &self.game_code
    }

    pub fn maker_code(&self) -> &str {
        &self.maker_code
    }

    pub fn software_version(&self) -> u8 {
        self.software_version
    }

    pub fn header_checksum(&self) -> u8 {
        self.header_checksum
    }

    fn parse(rom: &[u8]) -> Result<Self, LoadError> {
        if rom.len() < GBA_ROM_HEADER_MIN_SIZE {
            return Err(LoadError::RomTooSmall {
                minimum: GBA_ROM_HEADER_MIN_SIZE,
                actual: rom.len(),
            });
        }

        let fixed_value = rom[FIXED_VALUE_OFFSET];

        if fixed_value != EXPECTED_FIXED_VALUE {
            return Err(LoadError::InvalidRomFixedValue {
                expected: EXPECTED_FIXED_VALUE,
                actual: fixed_value,
            });
        }

        let expected_checksum = calculate_header_checksum(rom);
        let actual_checksum = rom[HEADER_CHECKSUM_OFFSET];

        if actual_checksum != expected_checksum {
            return Err(LoadError::InvalidRomHeaderChecksum {
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        Ok(Self {
            title: decode_ascii_field(&rom[TITLE_START..TITLE_END]),
            game_code: decode_ascii_field(&rom[GAME_CODE_START..GAME_CODE_END]),
            maker_code: decode_ascii_field(&rom[MAKER_CODE_START..MAKER_CODE_END]),
            software_version: rom[SOFTWARE_VERSION_OFFSET],
            header_checksum: actual_checksum,
        })
    }
}

pub fn load_bios_file(path: impl AsRef<Path>) -> Result<BiosImage, LoadError> {
    let path = path.as_ref();
    let bytes = read_file(path)?;

    if bytes.len() != BIOS_SIZE {
        return Err(LoadError::InvalidBiosSize {
            expected: BIOS_SIZE,
            actual: bytes.len(),
        });
    }

    Ok(BiosImage { bytes })
}

pub fn load_rom_file(path: impl AsRef<Path>) -> Result<RomImage, LoadError> {
    let path = path.as_ref();
    let bytes = read_file(path)?;

    if bytes.len() > GAME_PAK_ROM_MAX_SIZE {
        return Err(LoadError::RomTooLarge {
            maximum: GAME_PAK_ROM_MAX_SIZE,
            actual: bytes.len(),
        });
    }

    let header = RomHeader::parse(&bytes)?;

    Ok(RomImage { bytes, header })
}

fn read_file(path: &Path) -> Result<Vec<u8>, LoadError> {
    fs::read(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn calculate_header_checksum(rom: &[u8]) -> u8 {
    rom[TITLE_START..=SOFTWARE_VERSION_OFFSET]
        .iter()
        .fold(0u8, |checksum, byte| checksum.wrapping_sub(*byte))
        .wrapping_sub(0x19)
}

fn decode_ascii_field(bytes: &[u8]) -> String {
    let meaningful_length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    bytes[..meaningful_length]
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                char::from(*byte)
            } else {
                '\u{FFFD}'
            }
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, source: io::Error },

    InvalidBiosSize { expected: usize, actual: usize },

    RomTooSmall { minimum: usize, actual: usize },

    RomTooLarge { maximum: usize, actual: usize },

    InvalidRomFixedValue { expected: u8, actual: u8 },

    InvalidRomHeaderChecksum { expected: u8, actual: u8 },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read '{}': {source}", path.display())
            }

            Self::InvalidBiosSize { expected, actual } => {
                write!(
                    formatter,
                    "invalid GBA BIOS size: expected {expected} bytes, got {actual} bytes"
                )
            }

            Self::RomTooSmall { minimum, actual } => {
                write!(
                    formatter,
                    "ROM is too small: expected at least {minimum} bytes, got {actual} bytes"
                )
            }

            Self::RomTooLarge { maximum, actual } => {
                write!(
                    formatter,
                    "ROM is too large: maximum size is {maximum} bytes, got {actual} bytes"
                )
            }

            Self::InvalidRomFixedValue { expected, actual } => {
                write!(
                    formatter,
                    "invalid ROM header fixed value: expected 0x{expected:02X}, got 0x{actual:02X}"
                )
            }

            Self::InvalidRomHeaderChecksum { expected, actual } => {
                write!(
                    formatter,
                    "invalid ROM header checksum: expected 0x{expected:02X}, got 0x{actual:02X}"
                )
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FIXED_VALUE_OFFSET, HEADER_CHECKSUM_OFFSET, RomHeader, SOFTWARE_VERSION_OFFSET,
        calculate_header_checksum, decode_ascii_field,
    };

    fn create_test_rom() -> Vec<u8> {
        let mut rom = vec![0; 0xC0];

        rom[0xA0..0xAC].copy_from_slice(b"TEST GAME   ");
        rom[0xAC..0xB0].copy_from_slice(b"TEST");
        rom[0xB0..0xB2].copy_from_slice(b"01");

        rom[FIXED_VALUE_OFFSET] = 0x96;
        rom[SOFTWARE_VERSION_OFFSET] = 0;

        rom[HEADER_CHECKSUM_OFFSET] = calculate_header_checksum(&rom);

        rom
    }

    #[test]
    fn parses_valid_rom_header() {
        let rom = create_test_rom();

        let header = RomHeader::parse(&rom).unwrap();

        assert_eq!(header.title(), "TEST GAME");
        assert_eq!(header.game_code(), "TEST");
        assert_eq!(header.maker_code(), "01");
        assert_eq!(header.software_version(), 0);
    }

    #[test]
    fn rejects_invalid_fixed_value() {
        let mut rom = create_test_rom();

        rom[FIXED_VALUE_OFFSET] = 0;

        let result = RomHeader::parse(&rom);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_checksum() {
        let mut rom = create_test_rom();

        rom[HEADER_CHECKSUM_OFFSET] ^= 0xFF;

        let result = RomHeader::parse(&rom);

        assert!(result.is_err());
    }

    #[test]
    fn decodes_zero_terminated_ascii_field() {
        let text = decode_ascii_field(b"HELLO\0WORLD");

        assert_eq!(text, "HELLO");
    }
}
