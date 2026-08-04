const SRAM_SIZE: usize = 32 * 1024;
const LEGACY_SRAM_SIZE: usize = 64 * 1024;
const FLASH_512_SIZE: usize = 64 * 1024;
const FLASH_1M_SIZE: usize = 128 * 1024;
const FLASH_BANK_SIZE: usize = 64 * 1024;
const FLASH_SECTOR_SIZE: usize = 4 * 1024;
const EEPROM_512_SIZE: usize = 512;
const EEPROM_8K_SIZE: usize = 8 * 1024;

const FLASH_512_MAKER_ID: u8 = 0x32;
const FLASH_512_DEVICE_ID: u8 = 0x1B;
const FLASH_1M_MAKER_ID: u8 = 0x62;
const FLASH_1M_DEVICE_ID: u8 = 0x13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeSaveType {
    None,
    Sram,
    Flash512,
    Flash1M,
    EepromUnknown,
    Eeprom512,
    Eeprom8K,
}

impl CartridgeSaveType {
    pub const fn size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Sram => SRAM_SIZE,
            Self::Flash512 => FLASH_512_SIZE,
            Self::Flash1M => FLASH_1M_SIZE,
            Self::EepromUnknown | Self::Eeprom8K => EEPROM_8K_SIZE,
            Self::Eeprom512 => EEPROM_512_SIZE,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "No save hardware",
            Self::Sram => "SRAM 256K",
            Self::Flash512 => "Flash 512K",
            Self::Flash1M => "Flash 1M",
            Self::EepromUnknown => "EEPROM (size undetected)",
            Self::Eeprom512 => "EEPROM 512 B",
            Self::Eeprom8K => "EEPROM 8 KiB",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSaveLoadError {
    pub expected: usize,
    pub alternate_expected: Option<usize>,
    pub actual: usize,
}

impl std::fmt::Display for CartridgeSaveLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(alternate) = self.alternate_expected {
            write!(
                formatter,
                "invalid cartridge save size: expected {} or {} bytes, got {} bytes",
                self.expected, alternate, self.actual
            )
        } else {
            write!(
                formatter,
                "invalid cartridge save size: expected {} bytes, got {} bytes",
                self.expected, self.actual
            )
        }
    }
}

impl std::error::Error for CartridgeSaveLoadError {}

#[derive(Debug, Clone)]
enum CartridgeSaveStorage {
    None,
    Sram(Box<[u8; SRAM_SIZE]>),
    Flash(Flash),
    Eeprom(Eeprom),
}

#[derive(Debug, Clone)]
pub struct CartridgeSave {
    storage: CartridgeSaveStorage,
    dirty: bool,
}

impl CartridgeSave {
    pub fn from_rom(rom: &[u8]) -> Self {
        let storage = if contains_signature(rom, b"EEPROM_V") {
            CartridgeSaveStorage::Eeprom(Eeprom::new())
        } else if contains_signature(rom, b"FLASH1M_V") {
            CartridgeSaveStorage::Flash(Flash::new(FlashKind::Flash1M))
        } else if contains_signature(rom, b"FLASH512_V") || contains_signature(rom, b"FLASH_V") {
            CartridgeSaveStorage::Flash(Flash::new(FlashKind::Flash512))
        } else if contains_signature(rom, b"SRAM_V") || contains_signature(rom, b"SRAM_F_V") {
            CartridgeSaveStorage::Sram(Box::new([0xFF; SRAM_SIZE]))
        } else {
            CartridgeSaveStorage::None
        };

        Self {
            storage,
            dirty: false,
        }
    }

    pub const fn save_type(&self) -> CartridgeSaveType {
        match &self.storage {
            CartridgeSaveStorage::None => CartridgeSaveType::None,
            CartridgeSaveStorage::Sram(_) => CartridgeSaveType::Sram,
            CartridgeSaveStorage::Flash(flash) => flash.save_type(),
            CartridgeSaveStorage::Eeprom(eeprom) => eeprom.save_type(),
        }
    }

    pub fn data(&self) -> &[u8] {
        match &self.storage {
            CartridgeSaveStorage::None => &[],
            CartridgeSaveStorage::Sram(storage) => storage.as_slice(),
            CartridgeSaveStorage::Flash(flash) => flash.data(),
            CartridgeSaveStorage::Eeprom(eeprom) => eeprom.data(),
        }
    }

    pub fn load_data(&mut self, data: &[u8]) -> Result<(), CartridgeSaveLoadError> {
        let (expected, alternate_expected) = match &self.storage {
            CartridgeSaveStorage::Sram(_) => (SRAM_SIZE, Some(LEGACY_SRAM_SIZE)),
            CartridgeSaveStorage::Eeprom(_) => (EEPROM_512_SIZE, Some(EEPROM_8K_SIZE)),
            _ => (self.save_type().size(), None),
        };

        if data.len() != expected && Some(data.len()) != alternate_expected {
            return Err(CartridgeSaveLoadError {
                expected,
                alternate_expected,
                actual: data.len(),
            });
        }

        match &mut self.storage {
            CartridgeSaveStorage::None => {}
            CartridgeSaveStorage::Sram(storage) => {
                storage.copy_from_slice(&data[..SRAM_SIZE]);
            }
            CartridgeSaveStorage::Flash(flash) => flash.load_data(data),
            CartridgeSaveStorage::Eeprom(eeprom) => eeprom.load_data(data),
        }

        self.dirty = false;

        Ok(())
    }

    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn read8(&self, offset: usize) -> u8 {
        match &self.storage {
            CartridgeSaveStorage::None => 0xFF,
            CartridgeSaveStorage::Sram(storage) => storage[offset % SRAM_SIZE],
            CartridgeSaveStorage::Flash(flash) => flash.read8(offset),
            CartridgeSaveStorage::Eeprom(_) => 0xFF,
        }
    }

    pub fn write8(&mut self, offset: usize, value: u8) {
        let changed = match &mut self.storage {
            CartridgeSaveStorage::None => false,
            CartridgeSaveStorage::Sram(storage) => {
                let index = offset % SRAM_SIZE;
                let changed = storage[index] != value;
                storage[index] = value;
                changed
            }
            CartridgeSaveStorage::Flash(flash) => flash.write8(offset, value),
            CartridgeSaveStorage::Eeprom(_) => false,
        };

        self.dirty |= changed;
    }

    pub fn reset_protocol(&mut self) {
        if let CartridgeSaveStorage::Flash(flash) = &mut self.storage {
            flash.reset_protocol();
        }

        if let CartridgeSaveStorage::Eeprom(eeprom) = &mut self.storage {
            eeprom.reset_protocol();
        }
    }

    pub const fn is_eeprom(&self) -> bool {
        matches!(&self.storage, CartridgeSaveStorage::Eeprom(_))
    }

    pub fn begin_eeprom_dma(&mut self, transfer_count: u32, write: bool) {
        if let CartridgeSaveStorage::Eeprom(eeprom) = &mut self.storage {
            eeprom.begin_dma(transfer_count, write);
        }
    }

    pub fn write_eeprom_bit(&mut self, bit: bool) {
        let changed = match &mut self.storage {
            CartridgeSaveStorage::Eeprom(eeprom) => eeprom.write_bit(bit),
            _ => false,
        };

        self.dirty |= changed;
    }

    pub fn read_eeprom_bit(&mut self) -> bool {
        match &mut self.storage {
            CartridgeSaveStorage::Eeprom(eeprom) => eeprom.read_bit(),
            _ => true,
        }
    }
}

impl Default for CartridgeSave {
    fn default() -> Self {
        Self::from_rom(&[])
    }
}

fn contains_signature(bytes: &[u8], signature: &[u8]) -> bool {
    bytes
        .windows(signature.len())
        .any(|window| window == signature)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EepromSize {
    Unknown,
    Bytes512,
    Bytes8K,
}

impl EepromSize {
    const fn storage_size(self) -> usize {
        match self {
            Self::Unknown | Self::Bytes8K => EEPROM_8K_SIZE,
            Self::Bytes512 => EEPROM_512_SIZE,
        }
    }
}

#[derive(Debug, Clone)]
enum EepromTransfer {
    Idle,
    Receiving {
        bits: Vec<bool>,
        expected: usize,
        address_bits: usize,
    },
    Reading {
        bits: Vec<bool>,
        position: usize,
    },
}

#[derive(Debug, Clone)]
struct Eeprom {
    storage: Box<[u8; EEPROM_8K_SIZE]>,
    size: EepromSize,
    selected_block: usize,
    transfer: EepromTransfer,
}

impl Eeprom {
    fn new() -> Self {
        Self {
            storage: Box::new([0xFF; EEPROM_8K_SIZE]),
            size: EepromSize::Unknown,
            selected_block: 0,
            transfer: EepromTransfer::Idle,
        }
    }

    const fn save_type(&self) -> CartridgeSaveType {
        match self.size {
            EepromSize::Unknown => CartridgeSaveType::EepromUnknown,
            EepromSize::Bytes512 => CartridgeSaveType::Eeprom512,
            EepromSize::Bytes8K => CartridgeSaveType::Eeprom8K,
        }
    }

    fn data(&self) -> &[u8] {
        &self.storage[..self.size.storage_size()]
    }

    fn load_data(&mut self, data: &[u8]) {
        self.storage.fill(0xFF);
        self.storage[..data.len()].copy_from_slice(data);
        self.size = match data.len() {
            EEPROM_512_SIZE => EepromSize::Bytes512,
            EEPROM_8K_SIZE => EepromSize::Bytes8K,
            _ => unreachable!("validated EEPROM save size"),
        };
        self.reset_protocol();
    }

    fn begin_dma(&mut self, transfer_count: u32, write: bool) {
        if write {
            let address_bits = match transfer_count {
                9 | 73 => 6,
                17 | 81 => 14,
                _ => {
                    self.transfer = EepromTransfer::Idle;
                    return;
                }
            };

            self.detect_size(address_bits);
            self.transfer = EepromTransfer::Receiving {
                bits: Vec::with_capacity(transfer_count as usize),
                expected: transfer_count as usize,
                address_bits,
            };
        } else if transfer_count == 68 {
            self.prepare_read_data();
        } else {
            self.transfer = EepromTransfer::Idle;
        }
    }

    fn detect_size(&mut self, address_bits: usize) {
        match (self.size, address_bits) {
            (EepromSize::Unknown, 6) => self.size = EepromSize::Bytes512,
            (EepromSize::Unknown | EepromSize::Bytes512, 14) => {
                self.size = EepromSize::Bytes8K;
            }
            _ => {}
        }
    }

    fn write_bit(&mut self, bit: bool) -> bool {
        let complete = match &mut self.transfer {
            EepromTransfer::Receiving { bits, expected, .. } => {
                bits.push(bit);
                bits.len() == *expected
            }
            _ => return false,
        };

        if !complete {
            return false;
        }

        let transfer = std::mem::replace(&mut self.transfer, EepromTransfer::Idle);
        let EepromTransfer::Receiving {
            bits, address_bits, ..
        } = transfer
        else {
            unreachable!();
        };

        self.finish_command(&bits, address_bits)
    }

    fn finish_command(&mut self, bits: &[bool], address_bits: usize) -> bool {
        if bits.len() < 2 + address_bits + 1 || !bits[0] || bits.last() != Some(&false) {
            return false;
        }

        let address = bits_to_usize(&bits[2..2 + address_bits]);
        let block_mask = if address_bits == 6 { 0x3F } else { 0x3FF };
        self.selected_block = address & block_mask;

        if bits[1] {
            /* Read request: 11 + address + stop bit. */
            return false;
        }

        /* Write request: 10 + address + 64 data bits + stop bit. */
        let data_start = 2 + address_bits;
        let data_end = data_start + 64;

        if bits.len() < data_end + 1 {
            return false;
        }

        let storage_start = self.selected_block * 8;
        let mut changed = false;

        for byte_index in 0..8 {
            let bit_start = data_start + byte_index * 8;
            let value = bits_to_usize(&bits[bit_start..bit_start + 8]) as u8;
            let storage_index = storage_start + byte_index;

            changed |= self.storage[storage_index] != value;
            self.storage[storage_index] = value;
        }

        changed
    }

    fn prepare_read_data(&mut self) {
        let mut bits = Vec::with_capacity(68);

        /* Four dummy zero bits precede the 64 data bits. */
        bits.extend([false; 4]);

        let storage_start = self.selected_block * 8;

        for byte in &self.storage[storage_start..storage_start + 8] {
            for shift in (0..8).rev() {
                bits.push(byte & (1 << shift) != 0);
            }
        }

        self.transfer = EepromTransfer::Reading { bits, position: 0 };
    }

    fn read_bit(&mut self) -> bool {
        let EepromTransfer::Reading { bits, position } = &mut self.transfer else {
            /* Ready/status value after writes and outside a read transfer. */
            return true;
        };

        let bit = bits.get(*position).copied().unwrap_or(true);
        *position += 1;

        if *position >= bits.len() {
            self.transfer = EepromTransfer::Idle;
        }

        bit
    }

    fn reset_protocol(&mut self) {
        self.selected_block = 0;
        self.transfer = EepromTransfer::Idle;
    }
}

fn bits_to_usize(bits: &[bool]) -> usize {
    bits.iter()
        .fold(0usize, |value, &bit| (value << 1) | bit as usize)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashKind {
    Flash512,
    Flash1M,
}

impl FlashKind {
    const fn size(self) -> usize {
        match self {
            Self::Flash512 => FLASH_512_SIZE,
            Self::Flash1M => FLASH_1M_SIZE,
        }
    }

    const fn save_type(self) -> CartridgeSaveType {
        match self {
            Self::Flash512 => CartridgeSaveType::Flash512,
            Self::Flash1M => CartridgeSaveType::Flash1M,
        }
    }

    const fn ids(self) -> (u8, u8) {
        match self {
            Self::Flash512 => (FLASH_512_MAKER_ID, FLASH_512_DEVICE_ID),
            Self::Flash1M => (FLASH_1M_MAKER_ID, FLASH_1M_DEVICE_ID),
        }
    }

    const fn banked(self) -> bool {
        matches!(self, Self::Flash1M)
    }
}

#[derive(Debug, Clone)]
struct Flash {
    kind: FlashKind,
    storage: Vec<u8>,
    bank: usize,
    id_mode: bool,
    command_state: FlashCommandState,
}

impl Flash {
    fn new(kind: FlashKind) -> Self {
        Self {
            kind,
            storage: vec![0xFF; kind.size()],
            bank: 0,
            id_mode: false,
            command_state: FlashCommandState::Ready,
        }
    }

    pub fn read8(&self, offset: usize) -> u8 {
        let offset = offset % FLASH_BANK_SIZE;

        if self.id_mode {
            let (maker, device) = self.kind.ids();

            return match offset {
                0 => maker,
                1 => device,
                _ => 0xFF,
            };
        }

        self.storage[self.bank * FLASH_BANK_SIZE + offset]
    }

    pub fn write8(&mut self, offset: usize, value: u8) -> bool {
        let offset = offset % FLASH_BANK_SIZE;
        let mut storage_changed = false;

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
                        0xB0 if self.kind.banked() => FlashCommandState::SelectBank,
                        0x80 => FlashCommandState::EraseUnlock1,
                        _ => FlashCommandState::Ready,
                    }
                }
            }
            FlashCommandState::Program => {
                let index = self.bank * FLASH_BANK_SIZE + offset;
                let previous = self.storage[index];

                /* Flash programming can only clear bits. */
                self.storage[index] &= value;
                storage_changed = self.storage[index] != previous;

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
                    storage_changed = self.storage.iter().any(|&byte| byte != 0xFF);
                    self.storage.fill(0xFF);
                } else if value == 0x30 {
                    let bank_start = self.bank * FLASH_BANK_SIZE;
                    let sector_start =
                        bank_start + (offset / FLASH_SECTOR_SIZE) * FLASH_SECTOR_SIZE;

                    storage_changed = self.storage[sector_start..sector_start + FLASH_SECTOR_SIZE]
                        .iter()
                        .any(|&byte| byte != 0xFF);
                    self.storage[sector_start..sector_start + FLASH_SECTOR_SIZE].fill(0xFF);
                }

                FlashCommandState::Ready
            }
        };

        storage_changed
    }

    pub const fn data(&self) -> &[u8] {
        self.storage.as_slice()
    }

    const fn save_type(&self) -> CartridgeSaveType {
        self.kind.save_type()
    }

    pub fn load_data(&mut self, data: &[u8]) {
        debug_assert_eq!(data.len(), self.kind.size());
        self.storage.copy_from_slice(data);
        self.reset_protocol();
    }

    pub fn reset_protocol(&mut self) {
        self.bank = 0;
        self.id_mode = false;
        self.command_state = FlashCommandState::Ready;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CartridgeSave, CartridgeSaveType, FLASH_1M_DEVICE_ID, FLASH_1M_MAKER_ID,
        FLASH_512_DEVICE_ID, FLASH_512_MAKER_ID, Flash, FlashKind,
    };

    fn command(flash: &mut Flash, value: u8) {
        flash.write8(0x5555, 0xAA);
        flash.write8(0x2AAA, 0x55);
        flash.write8(0x5555, value);
    }

    fn program(flash: &mut Flash, offset: usize, value: u8) {
        command(flash, 0xA0);
        flash.write8(offset, value);
    }

    fn append_bits(bits: &mut Vec<bool>, value: usize, count: usize) {
        for shift in (0..count).rev() {
            bits.push(value & (1 << shift) != 0);
        }
    }

    fn eeprom_write(save: &mut CartridgeSave, address_bits: usize, block: usize, data: [u8; 8]) {
        let mut bits = vec![true, false];
        append_bits(&mut bits, block, address_bits);

        for byte in data {
            append_bits(&mut bits, byte as usize, 8);
        }

        bits.push(false);
        save.begin_eeprom_dma(bits.len() as u32, true);

        for bit in bits {
            save.write_eeprom_bit(bit);
        }
    }

    fn eeprom_read(save: &mut CartridgeSave, address_bits: usize, block: usize) -> Vec<bool> {
        let mut command = vec![true, true];
        append_bits(&mut command, block, address_bits);
        command.push(false);
        save.begin_eeprom_dma(command.len() as u32, true);

        for bit in command {
            save.write_eeprom_bit(bit);
        }

        save.begin_eeprom_dma(68, false);
        (0..68).map(|_| save.read_eeprom_bit()).collect()
    }

    #[test]
    fn flash_1m_signature_selects_flash_backend() {
        let save = CartridgeSave::from_rom(b"header FLASH1M_V103 trailer");

        assert_eq!(save.save_type(), CartridgeSaveType::Flash1M);
    }

    #[test]
    fn flash_512_signatures_select_64k_flash_backend() {
        for signature in [b"FLASH_V126".as_slice(), b"FLASH512_V133".as_slice()] {
            let save = CartridgeSave::from_rom(signature);

            assert_eq!(save.save_type(), CartridgeSaveType::Flash512);
            assert_eq!(save.data().len(), 64 * 1024);
        }
    }

    #[test]
    fn sram_signatures_select_32k_sram_backend() {
        for signature in [b"SRAM_V110".as_slice(), b"SRAM_F_V100".as_slice()] {
            let save = CartridgeSave::from_rom(signature);

            assert_eq!(save.save_type(), CartridgeSaveType::Sram);
            assert_eq!(save.data().len(), 32 * 1024);
        }
    }

    #[test]
    fn unknown_save_signature_selects_no_save_hardware() {
        let save = CartridgeSave::from_rom(b"no known save type");

        assert_eq!(save.save_type(), CartridgeSaveType::None);
        assert!(save.data().is_empty());
        assert_eq!(save.read8(0), 0xFF);
    }

    #[test]
    fn eeprom_signature_starts_with_undetected_size() {
        let save = CartridgeSave::from_rom(b"header EEPROM_V124 trailer");

        assert_eq!(save.save_type(), CartridgeSaveType::EepromUnknown);
        assert!(save.is_eeprom());
    }

    #[test]
    fn eeprom_512_write_and_read_use_msb_first_64_bit_blocks() {
        let mut save = CartridgeSave::from_rom(b"EEPROM_V124");
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

        eeprom_write(&mut save, 6, 0x2A, data);

        assert_eq!(save.save_type(), CartridgeSaveType::Eeprom512);
        assert!(save.is_dirty());
        assert_eq!(&save.data()[0x2A * 8..0x2A * 8 + 8], data);

        let bits = eeprom_read(&mut save, 6, 0x2A);
        assert_eq!(&bits[..4], [false; 4]);

        let decoded: Vec<u8> = bits[4..]
            .chunks_exact(8)
            .map(|bits| super::bits_to_usize(bits) as u8)
            .collect();
        assert_eq!(decoded, data);
    }

    #[test]
    fn eeprom_8k_uses_lower_ten_bits_of_fourteen_bit_address() {
        let mut save = CartridgeSave::from_rom(b"EEPROM_V124");
        let data = [0xA5; 8];

        eeprom_write(&mut save, 14, 0x0312, data);

        assert_eq!(save.save_type(), CartridgeSaveType::Eeprom8K);
        assert_eq!(&save.data()[0x0312 * 8..0x0312 * 8 + 8], data);
        assert_eq!(
            eeprom_read(&mut save, 14, 0x0312)[4..],
            [true, false, true, false, false, true, false, true].repeat(8)
        );
    }

    #[test]
    fn eeprom_save_file_size_selects_capacity() {
        let mut small = CartridgeSave::from_rom(b"EEPROM_V124");
        small.load_data(&[0xFF; 512]).unwrap();
        assert_eq!(small.save_type(), CartridgeSaveType::Eeprom512);

        let mut large = CartridgeSave::from_rom(b"EEPROM_V124");
        large.load_data(&[0xFF; 8 * 1024]).unwrap();
        assert_eq!(large.save_type(), CartridgeSaveType::Eeprom8K);

        let error = large.load_data(&[0; 1024]).unwrap_err();
        assert_eq!(error.expected, 512);
        assert_eq!(error.alternate_expected, Some(8 * 1024));
    }

    #[test]
    fn eeprom_reset_preserves_data_and_identical_writes_stay_clean() {
        let mut save = CartridgeSave::from_rom(b"EEPROM_V124");
        let data = [0x3C; 8];

        eeprom_write(&mut save, 6, 7, data);
        save.mark_clean();
        save.reset_protocol();

        assert_eq!(&save.data()[7 * 8..7 * 8 + 8], data);
        assert_eq!(save.save_type(), CartridgeSaveType::Eeprom512);

        eeprom_write(&mut save, 6, 7, data);
        assert!(!save.is_dirty());
    }

    #[test]
    fn invalid_eeprom_dma_length_does_not_select_a_capacity_or_write_data() {
        let mut save = CartridgeSave::from_rom(b"EEPROM_V124");

        save.begin_eeprom_dma(72, true);
        for _ in 0..72 {
            save.write_eeprom_bit(false);
        }

        assert_eq!(save.save_type(), CartridgeSaveType::EepromUnknown);
        assert!(!save.is_dirty());
        assert!(save.data().iter().all(|&byte| byte == 0xFF));
    }

    #[test]
    fn loaded_save_data_is_clean_and_requires_the_exact_backend_size() {
        let mut save = CartridgeSave::from_rom(b"FLASH1M_V103");
        let data = vec![0xA5; CartridgeSaveType::Flash1M.size()];

        save.load_data(&data).unwrap();

        assert_eq!(save.data(), data);
        assert!(!save.is_dirty());

        let error = save.load_data(&data[..data.len() - 1]).unwrap_err();
        assert_eq!(error.expected, CartridgeSaveType::Flash1M.size());
        assert_eq!(error.actual, CartridgeSaveType::Flash1M.size() - 1);
        assert_eq!(save.data(), data);
    }

    #[test]
    fn sram_accepts_legacy_64k_files_and_keeps_the_first_32k() {
        let mut save = CartridgeSave::from_rom(b"SRAM_V110");
        let mut legacy = vec![0xFF; 64 * 1024];
        legacy[0x1234] = 0x5A;
        legacy[32 * 1024 + 0x1234] = 0xA5;

        save.load_data(&legacy).unwrap();

        assert_eq!(save.data().len(), 32 * 1024);
        assert_eq!(save.data()[0x1234], 0x5A);
        assert!(!save.is_dirty());

        let error = save.load_data(&legacy[..legacy.len() - 1]).unwrap_err();
        assert_eq!(error.expected, 32 * 1024);
        assert_eq!(error.alternate_expected, Some(64 * 1024));
        assert_eq!(error.actual, 64 * 1024 - 1);
    }

    #[test]
    fn dirty_state_tracks_only_persistent_data_changes() {
        let mut save = CartridgeSave::from_rom(b"FLASH1M_V103");

        command(
            match &mut save.storage {
                super::CartridgeSaveStorage::Flash(flash) => flash,
                _ => unreachable!(),
            },
            0x90,
        );
        assert!(!save.is_dirty());

        save.write8(0x5555, 0xAA);
        save.write8(0x2AAA, 0x55);
        save.write8(0x5555, 0xA0);
        save.write8(0x1234, 0x5A);
        assert!(save.is_dirty());

        save.mark_clean();
        assert!(!save.is_dirty());

        save.write8(0x5555, 0xAA);
        save.write8(0x2AAA, 0x55);
        save.write8(0x5555, 0xA0);
        save.write8(0x1234, 0xFF);
        assert!(!save.is_dirty());
    }

    #[test]
    fn id_mode_reports_ids_for_both_flash_sizes() {
        let mut flash = Flash::new(FlashKind::Flash512);

        command(&mut flash, 0x90);

        assert_eq!(flash.read8(0), FLASH_512_MAKER_ID);
        assert_eq!(flash.read8(1), FLASH_512_DEVICE_ID);

        let mut flash = Flash::new(FlashKind::Flash1M);

        command(&mut flash, 0x90);

        assert_eq!(flash.read8(0), FLASH_1M_MAKER_ID);
        assert_eq!(flash.read8(1), FLASH_1M_DEVICE_ID);

        command(&mut flash, 0xF0);

        assert_eq!(flash.read8(0), 0xFF);
    }

    #[test]
    fn programming_and_sector_erase_follow_flash_semantics() {
        let mut flash = Flash::new(FlashKind::Flash512);

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
        let mut flash = Flash::new(FlashKind::Flash1M);

        program(&mut flash, 0x1234, 0x11);

        command(&mut flash, 0xB0);
        flash.write8(0, 1);
        program(&mut flash, 0x1234, 0x22);

        assert_eq!(flash.read8(0x1234), 0x22);

        command(&mut flash, 0xB0);
        flash.write8(0, 0);

        assert_eq!(flash.read8(0x1234), 0x11);
    }

    #[test]
    fn flash_512_ignores_bank_select_command() {
        let mut flash = Flash::new(FlashKind::Flash512);
        program(&mut flash, 0x1234, 0x11);

        command(&mut flash, 0xB0);
        flash.write8(0, 1);

        assert_eq!(flash.read8(0x1234), 0x11);
    }
}
