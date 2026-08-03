use super::{InterruptController, InterruptSource};

#[derive(Debug, Clone)]
pub struct IoRegisters {
    raw: Box<[u8; Self::SIZE]>,
    interrupts: InterruptController,
}

impl IoRegisters {
    pub const BASE: u32 = 0x0400_0000;
    pub const SIZE: usize = 0x400;

    pub const IE_OFFSET: u32 = 0x0200;
    pub const IF_OFFSET: u32 = 0x0202;
    pub const IME_OFFSET: u32 = 0x0208;

    pub fn new() -> Self {
        Self {
            raw: Box::new([0; Self::SIZE]),
            interrupts: InterruptController::new(),
        }
    }

    pub const fn contains_address(address: u32) -> bool {
        address >= Self::BASE && address < Self::BASE + Self::SIZE as u32
    }

    pub const fn address_to_offset(address: u32) -> u32 {
        address - Self::BASE
    }

    pub const fn interrupts(&self) -> &InterruptController {
        &self.interrupts
    }

    pub fn interrupts_mut(&mut self) -> &mut InterruptController {
        &mut self.interrupts
    }

    pub const fn irq_line(&self) -> bool {
        self.interrupts.irq_line()
    }

    pub fn request_interrupt(&mut self, source: InterruptSource) {
        self.interrupts.request(source);
    }

    pub fn reset(&mut self) {
        self.raw.fill(0);
        self.interrupts.reset();
    }

    pub fn read8(&self, offset: u32) -> u8 {
        match offset {
            Self::IE_OFFSET => self.interrupts.interrupt_enable() as u8,

            offset if offset == Self::IE_OFFSET + 1 => {
                (self.interrupts.interrupt_enable() >> 8) as u8
            }

            Self::IF_OFFSET => self.interrupts.interrupt_flags() as u8,

            offset if offset == Self::IF_OFFSET + 1 => {
                (self.interrupts.interrupt_flags() >> 8) as u8
            }

            Self::IME_OFFSET => self.interrupts.master_enable() as u8,

            offset if offset == Self::IME_OFFSET + 1 => 0,

            _ => self.read_raw8(offset),
        }
    }

    pub fn write8(&mut self, offset: u32, value: u8) {
        match offset {
            Self::IE_OFFSET => {
                let current = self.interrupts.interrupt_enable();

                let updated = (current & 0xFF00) | value as u16;

                self.interrupts.set_interrupt_enable(updated);
            }

            offset if offset == Self::IE_OFFSET + 1 => {
                let current = self.interrupts.interrupt_enable();

                let updated = (current & 0x00FF) | ((value as u16) << 8);

                self.interrupts.set_interrupt_enable(updated);
            }

            Self::IF_OFFSET => {
                self.interrupts.acknowledge(value as u16);
            }

            offset if offset == Self::IF_OFFSET + 1 => {
                self.interrupts.acknowledge((value as u16) << 8);
            }

            Self::IME_OFFSET => {
                self.interrupts.set_master_enable(value as u16);
            }

            offset if offset == Self::IME_OFFSET + 1 => {
                /*
                 * IME only exposes bit zero.
                 * Writes to its upper byte are ignored.
                 */
            }

            _ => {
                self.write_raw8(offset, value);
            }
        }
    }

    pub fn read16(&self, offset: u32) -> u16 {
        /*
         * GBA halfword accesses are aligned.
         */
        let offset = offset & !1;

        match offset {
            Self::IE_OFFSET => self.interrupts.interrupt_enable(),

            Self::IF_OFFSET => self.interrupts.interrupt_flags(),

            Self::IME_OFFSET => self.interrupts.master_enable() as u16,

            _ => {
                let low = self.read_raw8(offset);

                let high = self.read_raw8(offset.wrapping_add(1));

                u16::from_le_bytes([low, high])
            }
        }
    }

    pub fn write16(&mut self, offset: u32, value: u16) {
        let offset = offset & !1;

        match offset {
            Self::IE_OFFSET => {
                self.interrupts.set_interrupt_enable(value);
            }

            Self::IF_OFFSET => {
                self.interrupts.acknowledge(value);
            }

            Self::IME_OFFSET => {
                self.interrupts.set_master_enable(value);
            }

            _ => {
                let [low, high] = value.to_le_bytes();

                self.write_raw8(offset, low);

                self.write_raw8(offset.wrapping_add(1), high);
            }
        }
    }

    pub fn read32(&self, offset: u32) -> u32 {
        let offset = offset & !3;

        let low = self.read16(offset) as u32;

        let high = self.read16(offset.wrapping_add(2)) as u32;

        low | (high << 16)
    }

    pub fn write32(&mut self, offset: u32, value: u32) {
        let offset = offset & !3;

        /*
         * A word access to 0x200 covers both:
         *
         * IE at 0x200
         * IF at 0x202
         *
         * Calling write16 preserves each register's semantics,
         * including IF write-one-to-clear.
         */
        self.write16(offset, value as u16);

        self.write16(offset.wrapping_add(2), (value >> 16) as u16);
    }

    fn read_raw8(&self, offset: u32) -> u8 {
        self.raw.get(offset as usize).copied().unwrap_or(0)
    }

    fn write_raw8(&mut self, offset: u32, value: u8) {
        if let Some(byte) = self.raw.get_mut(offset as usize) {
            *byte = value;
        }
    }
}

impl Default for IoRegisters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::IoRegisters;

    use crate::bus::{InterruptController, InterruptSource};

    #[test]
    fn ie_halfword_read_write() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IE_OFFSET, 0x1234);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn ie_byte_access_is_little_endian() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::IE_OFFSET, 0x34);

        io.write8(IoRegisters::IE_OFFSET + 1, 0x12);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn if_write_one_to_clear_works_for_halfword() {
        let mut io = IoRegisters::new();

        io.request_interrupt(InterruptSource::VBlank);

        io.request_interrupt(InterruptSource::Timer0);

        io.write16(IoRegisters::IF_OFFSET, InterruptSource::VBlank.mask());

        assert_eq!(
            io.read16(IoRegisters::IF_OFFSET,),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn if_write_one_to_clear_works_for_bytes() {
        let mut io = IoRegisters::new();

        io.interrupts_mut().request_mask(0x1101);

        io.write8(IoRegisters::IF_OFFSET, 0x01);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0x1100);

        io.write8(IoRegisters::IF_OFFSET + 1, 0x10);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0x0100);
    }

    #[test]
    fn ime_only_uses_bit_zero() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IME_OFFSET, 0xFFFF);

        assert_eq!(io.read16(IoRegisters::IME_OFFSET,), 1);

        assert_eq!(io.read8(IoRegisters::IME_OFFSET + 1,), 0);
    }

    #[test]
    fn word_write_at_ie_also_acknowledges_if() {
        let mut io = IoRegisters::new();

        io.request_interrupt(InterruptSource::Timer0);

        /*
         * Low halfword:
         * IE <- Timer0
         *
         * High halfword:
         * IF acknowledge Timer0
         */
        let value =
            (InterruptSource::Timer0.mask() as u32) << 16 | InterruptSource::Timer0.mask() as u32;

        io.write32(IoRegisters::IE_OFFSET, value);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            InterruptSource::Timer0.mask()
        );

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0);
    }

    #[test]
    fn unimplemented_io_registers_use_backing_storage() {
        let mut io = IoRegisters::new();

        io.write16(0x0100, 0xCAFE);

        assert_eq!(io.read16(0x0100), 0xCAFE);
    }

    #[test]
    fn reset_clears_io_and_interrupt_state() {
        let mut io = IoRegisters::new();

        io.write16(0x0100, 0xCAFE);

        io.write16(IoRegisters::IE_OFFSET, InterruptSource::Timer0.mask());

        io.write16(IoRegisters::IME_OFFSET, 1);

        io.request_interrupt(InterruptSource::Timer0);

        assert!(io.irq_line());

        io.reset();

        assert_eq!(io.read16(0x0100), 0);

        assert_eq!(io.read16(IoRegisters::IE_OFFSET,), 0);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0);

        assert_eq!(io.read16(IoRegisters::IME_OFFSET,), 0);

        assert!(!io.irq_line());
    }
}
