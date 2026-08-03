use super::{
    DmaChannelIndex, DmaController, InterruptController, InterruptSource, TimerController,
    TimerIndex,
};

#[derive(Debug, Clone)]
pub struct IoRegisters {
    raw: Box<[u8; Self::SIZE]>,
    interrupts: InterruptController,
    timers: TimerController,
    dma: DmaController,
}

impl IoRegisters {
    pub const BASE: u32 = 0x0400_0000;
    pub const SIZE: usize = 0x400;

    pub const TM0CNT_L_OFFSET: u32 = 0x0100;
    pub const TM0CNT_H_OFFSET: u32 = 0x0102;

    pub const TM1CNT_L_OFFSET: u32 = 0x0104;
    pub const TM1CNT_H_OFFSET: u32 = 0x0106;

    pub const TM2CNT_L_OFFSET: u32 = 0x0108;
    pub const TM2CNT_H_OFFSET: u32 = 0x010A;

    pub const TM3CNT_L_OFFSET: u32 = 0x010C;
    pub const TM3CNT_H_OFFSET: u32 = 0x010E;

    pub const DMA0SAD_OFFSET: u32 = 0x00B0;
    pub const DMA0DAD_OFFSET: u32 = 0x00B4;
    pub const DMA0CNT_L_OFFSET: u32 = 0x00B8;
    pub const DMA0CNT_H_OFFSET: u32 = 0x00BA;

    pub const DMA1SAD_OFFSET: u32 = 0x00BC;
    pub const DMA1DAD_OFFSET: u32 = 0x00C0;
    pub const DMA1CNT_L_OFFSET: u32 = 0x00C4;
    pub const DMA1CNT_H_OFFSET: u32 = 0x00C6;

    pub const DMA2SAD_OFFSET: u32 = 0x00C8;
    pub const DMA2DAD_OFFSET: u32 = 0x00CC;
    pub const DMA2CNT_L_OFFSET: u32 = 0x00D0;
    pub const DMA2CNT_H_OFFSET: u32 = 0x00D2;

    pub const DMA3SAD_OFFSET: u32 = 0x00D4;
    pub const DMA3DAD_OFFSET: u32 = 0x00D8;
    pub const DMA3CNT_L_OFFSET: u32 = 0x00DC;
    pub const DMA3CNT_H_OFFSET: u32 = 0x00DE;

    pub const IE_OFFSET: u32 = 0x0200;
    pub const IF_OFFSET: u32 = 0x0202;
    pub const IME_OFFSET: u32 = 0x0208;

    pub fn new() -> Self {
        Self {
            raw: Box::new([0; Self::SIZE]),
            interrupts: InterruptController::new(),
            timers: TimerController::new(),
            dma: DmaController::new(),
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

    pub const fn timers(&self) -> &TimerController {
        &self.timers
    }

    pub fn timers_mut(&mut self) -> &mut TimerController {
        &mut self.timers
    }

    pub const fn dma(&self) -> &DmaController {
        &self.dma
    }

    pub fn dma_mut(&mut self) -> &mut DmaController {
        &mut self.dma
    }

    pub fn tick(&mut self, cycles: u32) {
        let interrupt_requests = self.timers.tick(cycles);

        if interrupt_requests != 0 {
            self.interrupts.request_mask(interrupt_requests);
        }
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
        self.timers.reset();
    }

    pub fn read8(&self, offset: u32) -> u8 {
        let aligned = offset & !1;

        if decode_dma_register(aligned).is_some()
            || decode_timer_register(aligned).is_some()
            || matches!(
                aligned,
                Self::IE_OFFSET | Self::IF_OFFSET | Self::IME_OFFSET
            )
        {
            let value = self.read16(aligned);

            return if offset & 1 == 0 {
                value as u8
            } else {
                (value >> 8) as u8
            };
        }

        self.read_raw8(offset)
    }

    pub fn write8(&mut self, offset: u32, value: u8) {
        let aligned = offset & !1;
        let high_byte = offset & 1 != 0;

        if let Some((channel, register)) = decode_dma_register(aligned) {
            let current = match register {
                DmaRegister::SourceLow => self.dma.read_source(channel) as u16,

                DmaRegister::SourceHigh => (self.dma.read_source(channel) >> 16) as u16,

                DmaRegister::DestinationLow => self.dma.read_destination(channel) as u16,

                DmaRegister::DestinationHigh => (self.dma.read_destination(channel) >> 16) as u16,

                DmaRegister::Count => self.dma.read_count(channel),

                DmaRegister::Control => self.dma.read_control(channel),
            };

            let updated = replace_byte(current, high_byte, value);

            self.write16(aligned, updated);

            return;
        }

        if let Some((timer, register)) = decode_timer_register(aligned) {
            let current = match register {
                TimerRegister::Counter => {
                    /*
                     * Counter reads expose the active counter, but writes
                     * modify the reload latch.
                     *
                     * Therefore byte merging must use reload, not the
                     * current counter.
                     */
                    self.timers.timer(timer).reload()
                }

                TimerRegister::Control => self.timers.read_control(timer),
            };

            let updated = replace_byte(current, high_byte, value);

            match register {
                TimerRegister::Counter => {
                    self.timers.write_reload(timer, updated);
                }

                TimerRegister::Control => {
                    self.timers.write_control(timer, updated);
                }
            }

            return;
        }

        match aligned {
            Self::IE_OFFSET => {
                let current = self.interrupts.interrupt_enable();

                let updated = replace_byte(current, high_byte, value);

                self.interrupts.set_interrupt_enable(updated);
            }

            Self::IF_OFFSET => {
                /*
                 * Byte-level write-one-to-clear.
                 */
                let mask = if high_byte {
                    (value as u16) << 8
                } else {
                    value as u16
                };

                self.interrupts.acknowledge(mask);
            }

            Self::IME_OFFSET => {
                /*
                 * Only bit zero of the low byte is meaningful.
                 */
                if !high_byte {
                    self.interrupts.set_master_enable(value as u16);
                }
            }

            _ => {
                self.write_raw8(offset, value);
            }
        }
    }

    pub fn read16(&self, offset: u32) -> u16 {
        let offset = offset & !1;

        if let Some((channel, register)) = decode_dma_register(offset) {
            return match register {
                DmaRegister::SourceLow => self.dma.read_source(channel) as u16,

                DmaRegister::SourceHigh => (self.dma.read_source(channel) >> 16) as u16,

                DmaRegister::DestinationLow => self.dma.read_destination(channel) as u16,

                DmaRegister::DestinationHigh => (self.dma.read_destination(channel) >> 16) as u16,

                /*
                 * DMA count registers are write-only on hardware.
                 * Returning zero is deterministic and suitable here.
                 */
                DmaRegister::Count => 0,

                DmaRegister::Control => self.dma.read_control(channel),
            };
        }

        if let Some((timer, register)) = decode_timer_register(offset) {
            return match register {
                TimerRegister::Counter => self.timers.read_counter(timer),

                TimerRegister::Control => self.timers.read_control(timer),
            };
        }

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

        if let Some((channel, register)) = decode_dma_register(offset) {
            match register {
                DmaRegister::SourceLow => {
                    let current = self.dma.read_source(channel);

                    let updated = (current & 0xFFFF_0000) | value as u32;

                    self.dma.write_source(channel, updated);
                }

                DmaRegister::SourceHigh => {
                    let current = self.dma.read_source(channel);

                    let updated = (current & 0x0000_FFFF) | ((value as u32) << 16);

                    self.dma.write_source(channel, updated);
                }

                DmaRegister::DestinationLow => {
                    let current = self.dma.read_destination(channel);

                    let updated = (current & 0xFFFF_0000) | value as u32;

                    self.dma.write_destination(channel, updated);
                }

                DmaRegister::DestinationHigh => {
                    let current = self.dma.read_destination(channel);

                    let updated = (current & 0x0000_FFFF) | ((value as u32) << 16);

                    self.dma.write_destination(channel, updated);
                }

                DmaRegister::Count => {
                    self.dma.write_count(channel, value);
                }

                DmaRegister::Control => {
                    self.dma.write_control(channel, value);
                }
            }

            return;
        }

        if let Some((timer, register)) = decode_timer_register(offset) {
            match register {
                TimerRegister::Counter => {
                    self.timers.write_reload(timer, value);
                }

                TimerRegister::Control => {
                    self.timers.write_control(timer, value);
                }
            }

            return;
        }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmaRegister {
    SourceLow,
    SourceHigh,
    DestinationLow,
    DestinationHigh,
    Count,
    Control,
}

fn decode_dma_register(offset: u32) -> Option<(DmaChannelIndex, DmaRegister)> {
    if !(IoRegisters::DMA0SAD_OFFSET..=IoRegisters::DMA3CNT_H_OFFSET).contains(&offset) {
        return None;
    }

    let relative = offset - IoRegisters::DMA0SAD_OFFSET;

    let channel_number = (relative / 12) as usize;

    if channel_number >= 4 {
        return None;
    }

    let register_offset = relative % 12;

    let register = match register_offset {
        0 => DmaRegister::SourceLow,
        2 => DmaRegister::SourceHigh,
        4 => DmaRegister::DestinationLow,
        6 => DmaRegister::DestinationHigh,
        8 => DmaRegister::Count,
        10 => DmaRegister::Control,
        _ => return None,
    };

    Some((DmaChannelIndex::from_usize(channel_number)?, register))
}

fn decode_timer_register(offset: u32) -> Option<(TimerIndex, TimerRegister)> {
    if !(IoRegisters::TM0CNT_L_OFFSET..=IoRegisters::TM3CNT_H_OFFSET).contains(&offset) {
        return None;
    }

    let relative = offset - IoRegisters::TM0CNT_L_OFFSET;

    let timer_number = (relative / 4) as usize;

    let register = match relative % 4 {
        0 | 1 => TimerRegister::Counter,
        2 | 3 => TimerRegister::Control,
        _ => unreachable!(),
    };

    Some((TimerIndex::from_usize(timer_number)?, register))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerRegister {
    Counter,
    Control,
}

const fn replace_byte(original: u16, high_byte: bool, value: u8) -> u16 {
    if high_byte {
        (original & 0x00FF) | ((value as u16) << 8)
    } else {
        (original & 0xFF00) | value as u16
    }
}

#[cfg(test)]
mod tests {
    use super::IoRegisters;

    use crate::bus::{
        DmaChannelIndex, DmaTransferWidth, InterruptController, InterruptSource, TimerIndex,
    };

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

        const UNIMPLEMENTED_OFFSET: u32 = 0x0300;

        io.write16(UNIMPLEMENTED_OFFSET, 0xCAFE);

        assert_eq!(io.read16(UNIMPLEMENTED_OFFSET), 0xCAFE,);
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

    #[test]
    fn timer_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFF0);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0xFFF0);

        assert_eq!(io.read16(IoRegisters::TM0CNT_H_OFFSET,), 1 << 7);
    }

    #[test]
    fn io_tick_advances_timer() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        io.tick(42);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 42);
    }

    #[test]
    fn timer_overflow_sets_if() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFFF);

        /*
         * Enable + IRQ.
         */
        io.write16(IoRegisters::TM0CNT_H_OFFSET, (1 << 7) | (1 << 6));

        io.tick(1);

        assert_eq!(
            io.read16(IoRegisters::IF_OFFSET) & InterruptSource::Timer0.mask(),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn byte_writes_to_timer_reload_merge_against_reload_latch() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::TM0CNT_L_OFFSET, 0x34);

        io.write8(IoRegisters::TM0CNT_L_OFFSET + 1, 0x12);

        /*
         * Enable loads reload into the active counter.
         */
        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0x1234);
    }

    #[test]
    fn timer_irq_can_raise_irq_line() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IE_OFFSET, InterruptSource::Timer0.mask());

        io.write16(IoRegisters::IME_OFFSET, 1);

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFFF);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, (1 << 7) | (1 << 6));

        assert!(!io.irq_line());

        io.tick(1);

        assert!(io.irq_line());
    }

    #[test]
    fn writing_timer_counter_updates_reload_latch_not_active_counter() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xCAFE);

        /*
         * TM0CNT_L reads the current counter.
         * Writing while disabled only updates the reload latch.
         */
        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0,);

        assert_eq!(io.timers().timer(TimerIndex::Timer0).reload(), 0xCAFE,);
    }

    #[test]
    fn enabling_timer_loads_reload_latch_into_counter() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xCAFE);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0xCAFE,);
    }

    #[test]
    fn dma_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write32(IoRegisters::DMA0SAD_OFFSET, 0x0200_0100);

        io.write32(IoRegisters::DMA0DAD_OFFSET, 0x0300_0200);

        io.write16(IoRegisters::DMA0CNT_L_OFFSET, 16);

        assert_eq!(io.dma().read_source(DmaChannelIndex::Dma0), 0x0200_0100);

        assert_eq!(
            io.dma().read_destination(DmaChannelIndex::Dma0),
            0x0300_0200
        );

        assert_eq!(io.dma().read_count(DmaChannelIndex::Dma0), 16);
    }

    #[test]
    fn enabling_immediate_dma_marks_channel_pending() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DMA0CNT_L_OFFSET, 4);

        io.write16(IoRegisters::DMA0CNT_H_OFFSET, 1 << 15);

        assert!(io.dma().channel(DmaChannelIndex::Dma0).pending());
    }

    #[test]
    fn dma_word_write_can_set_count_and_control() {
        let mut io = IoRegisters::new();

        let count = 4u32;
        let control = ((1 << 15) | (1 << 10)) as u32;

        io.write32(IoRegisters::DMA0CNT_L_OFFSET, count | (control << 16));

        let request = io.dma_mut().next_pending_request().unwrap();

        assert_eq!(request.count, 4);

        assert_eq!(request.width, DmaTransferWidth::Word);
    }
}
