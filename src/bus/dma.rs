use super::InterruptSource;

pub const DMA_CHANNEL_COUNT: usize = 4;
const FIFO_A_ADDRESS: u32 = 0x0400_00A0;
const FIFO_B_ADDRESS: u32 = 0x0400_00A4;

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

    /*
     * Number of queued DMA activations.
     *
     * A counter is safer than bool if one peripheral tick crosses more
     * than one HBlank event.
     */
    pending_count: u32,
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

            pending_count: 0,
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
        self.pending_count != 0
    }

    pub const fn pending_count(self) -> u32 {
        self.pending_count
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
    pub start_timing: DmaStartTiming,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaTransferCompletion {
    pub channel: DmaChannelIndex,
    pub final_source: u32,
    pub final_destination: u32,
    pub transferred_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    channels: [DmaChannel; DMA_CHANNEL_COUNT],
    pending_mask: u8,
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
            pending_mask: 0,
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
        let channel_number = index.as_usize();
        let channel = &mut self.channels[channel_number];

        let old_enabled = channel.control.enabled();

        let new_control = DmaControl::from_raw(value);

        let new_enabled = new_control.enabled();

        channel.control = new_control;

        if !old_enabled && new_enabled {
            Self::latch_channel(index, channel);

            if new_control.start_timing() == DmaStartTiming::Immediate {
                channel.pending_count = channel.pending_count.saturating_add(1);
                self.pending_mask |= 1 << channel_number;
            }
        }

        if old_enabled && !new_enabled {
            channel.pending_count = 0;
            self.pending_mask &= !(1 << channel_number);
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
        if self.pending_mask == 0 {
            return None;
        }

        /* The least-significant set bit is the highest-priority channel. */
        let channel_number = self.pending_mask.trailing_zeros() as usize;
        let channel_index =
            DmaChannelIndex::from_usize(channel_number).expect("pending DMA channel is valid");
        let channel = &mut self.channels[channel_number];

        debug_assert!(channel.pending_count != 0 && channel.control.enabled());
        channel.pending_count -= 1;

        if channel.pending_count == 0 {
            self.pending_mask &= !(1 << channel_number);
        }

        let sound_fifo_dma = matches!(channel_index, DmaChannelIndex::Dma1 | DmaChannelIndex::Dma2)
            && channel.control.start_timing() == DmaStartTiming::Special
            && matches!(
                channel.internal_destination,
                FIFO_A_ADDRESS | FIFO_B_ADDRESS
            );

        Some(DmaTransferRequest {
            channel: channel_index,
            source: channel.internal_source,
            destination: channel.internal_destination,
            count: if sound_fifo_dma {
                4
            } else {
                channel.internal_count
            },
            width: if sound_fifo_dma {
                DmaTransferWidth::Word
            } else {
                channel.control.transfer_width()
            },

            source_control: match channel.control.source_control() {
                /*
                 * Source mode 3 is prohibited. Treat it deterministically as
                 * increment until an explicit invalid-DMA policy is added.
                 */
                DmaAddressControl::IncrementReload => DmaAddressControl::Increment,

                control => control,
            },

            destination_control: if sound_fifo_dma {
                DmaAddressControl::Fixed
            } else {
                channel.control.destination_control()
            },

            irq_enabled: channel.control.irq_enabled(),

            start_timing: channel.control.start_timing(),

            repeat: channel.control.repeat(),
        })
    }

    pub fn complete_transfer(&mut self, completion: DmaTransferCompletion) {
        let index = completion.channel;
        let channel = &mut self.channels[index.as_usize()];

        let repeated_event_dma =
            channel.control.repeat() && channel.control.start_timing() != DmaStartTiming::Immediate;

        channel.internal_source = completion.final_source;

        if repeated_event_dma {
            channel.internal_destination = match channel.control.destination_control() {
                DmaAddressControl::IncrementReload => {
                    channel.destination_register & index.destination_address_mask()
                }

                _ => completion.final_destination,
            };

            let encoded_count = channel.count_register & index.count_mask();

            channel.internal_count = if encoded_count == 0 {
                index.maximum_count()
            } else {
                encoded_count as u32
            };

            /*
             * Enable remains set while waiting for another event.
             *
             * Do not clear pending_count: another HBlank/VBlank may have
             * been queued while the previous transfer occupied the bus.
             */
        } else {
            channel.internal_destination = completion.final_destination;

            channel.internal_count = 0;
            channel.control.set_enabled(false);
            channel.pending_count = 0;
            self.pending_mask &= !(1 << index.as_usize());
        }
    }

    pub fn trigger(&mut self, timing: DmaStartTiming, occurrences: u32) {
        if occurrences == 0 || timing == DmaStartTiming::Immediate {
            return;
        }

        for (channel_number, channel) in self.channels.iter_mut().enumerate() {
            if !channel.control.enabled() {
                continue;
            }

            if channel.control.start_timing() != timing {
                continue;
            }

            channel.pending_count = channel.pending_count.saturating_add(occurrences);
            self.pending_mask |= 1 << channel_number;
        }
    }

    pub fn trigger_sound_fifo(&mut self, destination: u32) {
        if !matches!(destination, FIFO_A_ADDRESS | FIFO_B_ADDRESS) {
            return;
        }

        for channel_index in [DmaChannelIndex::Dma1, DmaChannelIndex::Dma2] {
            let channel_number = channel_index.as_usize();
            let channel = &mut self.channels[channel_number];

            if !channel.control.enabled()
                || channel.control.start_timing() != DmaStartTiming::Special
                || channel.internal_destination != destination
            {
                continue;
            }

            channel.pending_count = channel.pending_count.saturating_add(1);
            self.pending_mask |= 1 << channel_number;
        }
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.pending_mask = 0;
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
        DmaAddressControl, DmaChannelIndex, DmaController, DmaStartTiming, DmaTransferCompletion,
        DmaTransferWidth, FIFO_A_ADDRESS,
    };

    #[test]
    fn sound_fifo_dma_is_forced_to_four_fixed_words() {
        let mut dma = DmaController::new();

        dma.write_source(DmaChannelIndex::Dma1, 0x0200_0000);
        dma.write_destination(DmaChannelIndex::Dma1, FIFO_A_ADDRESS);
        dma.write_count(DmaChannelIndex::Dma1, 99);
        dma.write_control(DmaChannelIndex::Dma1, (1 << 9) | (0b11 << 12) | (1 << 15));

        dma.trigger_sound_fifo(FIFO_A_ADDRESS);
        let request = dma
            .next_pending_request()
            .expect("sound DMA must be pending");

        assert_eq!(request.count, 4);
        assert_eq!(request.width, DmaTransferWidth::Word);
        assert_eq!(request.destination_control, DmaAddressControl::Fixed);
    }

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

    #[test]
    fn vblank_dma_becomes_pending_on_vblank_event() {
        let mut dma = DmaController::new();

        dma.write_count(DmaChannelIndex::Dma0, 4);

        /*
         * VBlank timing + enable.
         */
        dma.write_control(DmaChannelIndex::Dma0, (0b01 << 12) | (1 << 15));

        assert!(dma.next_pending_request().is_none());

        dma.trigger(DmaStartTiming::VBlank, 1);

        let request = dma.next_pending_request().unwrap();

        assert_eq!(request.channel, DmaChannelIndex::Dma0,);

        assert_eq!(request.start_timing, DmaStartTiming::VBlank,);
    }

    #[test]
    fn hblank_event_only_triggers_hblank_dma() {
        let mut dma = DmaController::new();

        dma.write_control(DmaChannelIndex::Dma0, (0b01 << 12) | (1 << 15));

        dma.write_control(DmaChannelIndex::Dma1, (0b10 << 12) | (1 << 15));

        dma.trigger(DmaStartTiming::HBlank, 1);

        assert_eq!(
            dma.next_pending_request().unwrap().channel,
            DmaChannelIndex::Dma1,
        );

        assert!(dma.next_pending_request().is_none());
    }

    #[test]
    fn repeat_event_dma_remains_enabled_after_completion() {
        let mut dma = DmaController::new();

        dma.write_source(DmaChannelIndex::Dma0, 0x0200_0000);

        dma.write_destination(DmaChannelIndex::Dma0, 0x0300_0000);

        dma.write_count(DmaChannelIndex::Dma0, 4);

        /*
         * Repeat + VBlank + enable.
         */
        dma.write_control(DmaChannelIndex::Dma0, (1 << 9) | (0b01 << 12) | (1 << 15));

        dma.trigger(DmaStartTiming::VBlank, 1);

        let request = dma.next_pending_request().unwrap();

        dma.complete_transfer(DmaTransferCompletion {
            channel: request.channel,
            final_source: 0x0200_0008,
            final_destination: 0x0300_0008,
            transferred_units: 4,
        });

        assert!(dma.channel(DmaChannelIndex::Dma0).control().enabled());

        dma.trigger(DmaStartTiming::VBlank, 1);

        let second = dma.next_pending_request().unwrap();

        assert_eq!(second.source, 0x0200_0008);
        assert_eq!(second.destination, 0x0300_0008);
        assert_eq!(second.count, 4);
    }

    #[test]
    fn destination_increment_reload_restores_destination() {
        let mut dma = DmaController::new();

        dma.write_source(DmaChannelIndex::Dma0, 0x0200_0000);

        dma.write_destination(DmaChannelIndex::Dma0, 0x0300_0000);

        dma.write_count(DmaChannelIndex::Dma0, 4);

        /*
         * Destination increment/reload
         * Repeat
         * VBlank
         * Enable
         */
        dma.write_control(
            DmaChannelIndex::Dma0,
            (0b11 << 5) | (1 << 9) | (0b01 << 12) | (1 << 15),
        );

        dma.trigger(DmaStartTiming::VBlank, 1);

        let request = dma.next_pending_request().unwrap();

        dma.complete_transfer(DmaTransferCompletion {
            channel: request.channel,
            final_source: 0x0200_0008,
            final_destination: 0x0300_0008,
            transferred_units: 4,
        });

        dma.trigger(DmaStartTiming::VBlank, 1);

        let repeated = dma.next_pending_request().unwrap();

        assert_eq!(repeated.source, 0x0200_0008,);

        assert_eq!(repeated.destination, 0x0300_0000,);
    }
}
