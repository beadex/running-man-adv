use super::InterruptSource;

pub const DMA_CHANNEL_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaChannelIndex {
    Dma0 = 0,
    Dma1 = 1,
    Dma2 = 2,
    Dma3 = 3,
}

impl DmaChannelIndex {
    pub const fn from_usize(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Dma0),
            1 => Some(Self::Dma1),
            2 => Some(Self::Dma2),
            3 => Some(Self::Dma3),
            _ => None,
        }
    }

    pub const fn as_usize(self) -> usize {
        self as usize
    }

    pub const fn interrupt_source(self) -> InterruptSource {
        match self {
            Self::Dma0 => InterruptSource::Dma0,
            Self::Dma1 => InterruptSource::Dma1,
            Self::Dma2 => InterruptSource::Dma2,
            Self::Dma3 => InterruptSource::Dma3,
        }
    }

    pub const fn maximum_count(self) -> u32 {
        match self {
            Self::Dma0 | Self::Dma1 | Self::Dma2 => 0x4000,
            Self::Dma3 => 0x1_0000,
        }
    }

    pub const fn count_mask(self) -> u16 {
        match self {
            Self::Dma0 | Self::Dma1 | Self::Dma2 => 0x3FFF,
            Self::Dma3 => 0xFFFF,
        }
    }

    pub const fn source_address_mask(self) -> u32 {
        match self {
            /*
             * DMA0 source cannot access the cartridge address range.
             */
            Self::Dma0 => 0x07FF_FFFF,

            Self::Dma1 | Self::Dma2 | Self::Dma3 => 0x0FFF_FFFF,
        }
    }

    pub const fn destination_address_mask(self) -> u32 {
        match self {
            Self::Dma0 | Self::Dma1 | Self::Dma2 => 0x07FF_FFFF,

            Self::Dma3 => 0x0FFF_FFFF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaAddressControl {
    Increment,
    Decrement,
    Fixed,

    /*
     * Valid for destination only.
     *
     * During transfer it increments normally. For repeated,
     * event-triggered DMA, the destination is restored afterward.
     */
    IncrementReload,
}

impl DmaAddressControl {
    pub const fn from_bits(bits: u16) -> Self {
        match bits & 0b11 {
            0b00 => Self::Increment,
            0b01 => Self::Decrement,
            0b10 => Self::Fixed,
            0b11 => Self::IncrementReload,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStartTiming {
    Immediate,
    VBlank,
    HBlank,
    Special,
}

impl DmaStartTiming {
    pub const fn from_bits(bits: u16) -> Self {
        match bits & 0b11 {
            0b00 => Self::Immediate,
            0b01 => Self::VBlank,
            0b10 => Self::HBlank,
            0b11 => Self::Special,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaTransferWidth {
    Halfword,
    Word,
}

impl DmaTransferWidth {
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Halfword => 2,
            Self::Word => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaControl {
    raw: u16,
}

impl DmaControl {
    /*
     * Destination control: bits 5..6
     * Source control:      bits 7..8
     * Repeat:              bit 9
     * Width:               bit 10
     * Game Pak DRQ:        bit 11
     * Start timing:        bits 12..13
     * IRQ:                 bit 14
     * Enable:              bit 15
     */
    pub const WRITABLE_MASK: u16 = 0xFFE0;

    pub const fn new() -> Self {
        Self { raw: 0 }
    }

    pub const fn from_raw(raw: u16) -> Self {
        Self {
            raw: raw & Self::WRITABLE_MASK,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub const fn destination_control(self) -> DmaAddressControl {
        DmaAddressControl::from_bits((self.raw >> 5) & 0b11)
    }

    pub const fn source_control(self) -> DmaAddressControl {
        DmaAddressControl::from_bits((self.raw >> 7) & 0b11)
    }

    pub const fn repeat(self) -> bool {
        self.raw & (1 << 9) != 0
    }

    pub const fn transfer_width(self) -> DmaTransferWidth {
        if self.raw & (1 << 10) != 0 {
            DmaTransferWidth::Word
        } else {
            DmaTransferWidth::Halfword
        }
    }

    pub const fn game_pak_drq(self) -> bool {
        self.raw & (1 << 11) != 0
    }

    pub const fn start_timing(self) -> DmaStartTiming {
        DmaStartTiming::from_bits((self.raw >> 12) & 0b11)
    }

    pub const fn irq_enabled(self) -> bool {
        self.raw & (1 << 14) != 0
    }

    pub const fn enabled(self) -> bool {
        self.raw & (1 << 15) != 0
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 15;
        } else {
            self.raw &= !(1 << 15);
        }
    }
}

impl Default for DmaControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaChannel {
    source_register: u32,
    destination_register: u32,
    count_register: u16,
    control: DmaControl,

    /*
     * Internal latched values used while a transfer is active.
     */
    internal_source: u32,
    internal_destination: u32,
    internal_count: u32,

    pending: bool,
}

impl DmaChannel {
    pub const fn new() -> Self {
        Self {
            source_register: 0,
            destination_register: 0,
            count_register: 0,
            control: DmaControl::new(),

            internal_source: 0,
            internal_destination: 0,
            internal_count: 0,

            pending: false,
        }
    }

    pub const fn source_register(self) -> u32 {
        self.source_register
    }

    pub const fn destination_register(self) -> u32 {
        self.destination_register
    }

    pub const fn count_register(self) -> u16 {
        self.count_register
    }

    pub const fn control(self) -> DmaControl {
        self.control
    }

    pub const fn pending(self) -> bool {
        self.pending
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaTransferRequest {
    pub channel: DmaChannelIndex,
    pub source: u32,
    pub destination: u32,
    pub count: u32,
    pub width: DmaTransferWidth,
    pub source_control: DmaAddressControl,
    pub destination_control: DmaAddressControl,
    pub irq_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaTransferCompletion {
    pub channel: DmaChannelIndex,
    pub final_source: u32,
    pub final_destination: u32,
    pub transferred_units: u32,
    pub request_interrupt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    channels: [DmaChannel; DMA_CHANNEL_COUNT],
}

impl DmaController {
    pub const fn new() -> Self {
        Self {
            channels: [
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
            ],
        }
    }

    pub fn channel(&self, index: DmaChannelIndex) -> &DmaChannel {
        &self.channels[index.as_usize()]
    }

    pub fn read_source(&self, index: DmaChannelIndex) -> u32 {
        self.channel(index).source_register()
    }

    pub fn read_destination(&self, index: DmaChannelIndex) -> u32 {
        self.channel(index).destination_register()
    }

    pub fn read_count(&self, index: DmaChannelIndex) -> u16 {
        self.channel(index).count_register()
    }

    pub fn read_control(&self, index: DmaChannelIndex) -> u16 {
        self.channel(index).control().raw()
    }

    pub fn write_source(&mut self, index: DmaChannelIndex, value: u32) {
        let mask = index.source_address_mask();

        self.channels[index.as_usize()].source_register = value & mask;
    }

    pub fn write_destination(&mut self, index: DmaChannelIndex, value: u32) {
        let mask = index.destination_address_mask();

        self.channels[index.as_usize()].destination_register = value & mask;
    }

    pub fn write_count(&mut self, index: DmaChannelIndex, value: u16) {
        self.channels[index.as_usize()].count_register = value & index.count_mask();
    }

    pub fn write_control(&mut self, index: DmaChannelIndex, value: u16) {
        let channel = &mut self.channels[index.as_usize()];

        let old_enabled = channel.control.enabled();
        let new_control = DmaControl::from_raw(value);
        let new_enabled = new_control.enabled();

        channel.control = new_control;

        if !old_enabled && new_enabled {
            Self::latch_channel(index, channel);

            if new_control.start_timing() == DmaStartTiming::Immediate {
                channel.pending = true;
            }
        }

        if old_enabled && !new_enabled {
            channel.pending = false;
        }
    }

    fn latch_channel(index: DmaChannelIndex, channel: &mut DmaChannel) {
        channel.internal_source = channel.source_register & index.source_address_mask();

        channel.internal_destination =
            channel.destination_register & index.destination_address_mask();

        let encoded_count = channel.count_register & index.count_mask();

        channel.internal_count = if encoded_count == 0 {
            index.maximum_count()
        } else {
            encoded_count as u32
        };
    }

    pub fn next_pending_request(&mut self) -> Option<DmaTransferRequest> {
        /*
         * Lower numbered channel has higher priority.
         */
        for channel_number in 0..DMA_CHANNEL_COUNT {
            let channel_index =
                DmaChannelIndex::from_usize(channel_number).expect("valid DMA channel");

            let channel = &mut self.channels[channel_number];

            if !channel.pending || !channel.control.enabled() {
                continue;
            }

            channel.pending = false;

            return Some(DmaTransferRequest {
                channel: channel_index,
                source: channel.internal_source,
                destination: channel.internal_destination,
                count: channel.internal_count,
                width: channel.control.transfer_width(),
                source_control: channel.control.source_control(),
                destination_control: channel.control.destination_control(),
                irq_enabled: channel.control.irq_enabled(),
            });
        }

        None
    }

    pub fn complete_transfer(&mut self, completion: DmaTransferCompletion) {
        let channel = &mut self.channels[completion.channel.as_usize()];

        channel.internal_source = completion.final_source;

        channel.internal_destination = completion.final_destination;

        channel.internal_count = 0;

        /*
         * Immediate DMA always disables after completion.
         *
         * Repeat applies to event-triggered DMA and will be added
         * when VBlank/HBlank/Special triggering exists.
         */
        channel.control.set_enabled(false);
        channel.pending = false;
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }
}

impl Default for DmaController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DmaAddressControl, DmaChannelIndex, DmaController, DmaStartTiming, DmaTransferWidth,
    };

    #[test]
    fn control_fields_are_decoded() {
        let mut dma = DmaController::new();

        let control =
            (0b01 << 5) | (0b10 << 7) | (1 << 9) | (1 << 10) | (0b10 << 12) | (1 << 14) | (1 << 15);

        dma.write_control(DmaChannelIndex::Dma0, control);

        let decoded = dma.channel(DmaChannelIndex::Dma0).control();

        assert_eq!(decoded.destination_control(), DmaAddressControl::Decrement);

        assert_eq!(decoded.source_control(), DmaAddressControl::Fixed);

        assert!(decoded.repeat());

        assert_eq!(decoded.transfer_width(), DmaTransferWidth::Word);

        assert_eq!(decoded.start_timing(), DmaStartTiming::HBlank);

        assert!(decoded.irq_enabled());
        assert!(decoded.enabled());
    }

    #[test]
    fn immediate_dma_becomes_pending_on_enable_edge() {
        let mut dma = DmaController::new();

        dma.write_source(DmaChannelIndex::Dma0, 0x0200_0000);

        dma.write_destination(DmaChannelIndex::Dma0, 0x0300_0000);

        dma.write_count(DmaChannelIndex::Dma0, 4);

        dma.write_control(DmaChannelIndex::Dma0, 1 << 15);

        assert!(dma.channel(DmaChannelIndex::Dma0).pending());

        let request = dma.next_pending_request().unwrap();

        assert_eq!(request.count, 4);

        assert_eq!(request.source, 0x0200_0000);

        assert_eq!(request.destination, 0x0300_0000);
    }

    #[test]
    fn zero_count_uses_channel_maximum() {
        let mut dma = DmaController::new();

        dma.write_count(DmaChannelIndex::Dma0, 0);

        dma.write_control(DmaChannelIndex::Dma0, 1 << 15);

        assert_eq!(dma.next_pending_request().unwrap().count, 0x4000);

        dma.write_count(DmaChannelIndex::Dma3, 0);

        dma.write_control(DmaChannelIndex::Dma3, 1 << 15);

        assert_eq!(dma.next_pending_request().unwrap().count, 0x1_0000);
    }

    #[test]
    fn lower_channel_has_higher_priority() {
        let mut dma = DmaController::new();

        dma.write_control(DmaChannelIndex::Dma2, 1 << 15);

        dma.write_control(DmaChannelIndex::Dma0, 1 << 15);

        assert_eq!(
            dma.next_pending_request().unwrap().channel,
            DmaChannelIndex::Dma0
        );

        assert_eq!(
            dma.next_pending_request().unwrap().channel,
            DmaChannelIndex::Dma2
        );
    }

    #[test]
    fn non_immediate_dma_does_not_start_yet() {
        let mut dma = DmaController::new();

        /*
         * Start timing VBlank.
         */
        dma.write_control(DmaChannelIndex::Dma0, (0b01 << 12) | (1 << 15));

        assert!(dma.next_pending_request().is_none());
    }
}
