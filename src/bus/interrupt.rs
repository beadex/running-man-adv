#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum InterruptSource {
    VBlank = 1 << 0,
    HBlank = 1 << 1,
    VCounterMatch = 1 << 2,
    Timer0 = 1 << 3,
    Timer1 = 1 << 4,
    Timer2 = 1 << 5,
    Timer3 = 1 << 6,
    Serial = 1 << 7,
    Dma0 = 1 << 8,
    Dma1 = 1 << 9,
    Dma2 = 1 << 10,
    Dma3 = 1 << 11,
    Keypad = 1 << 12,
    GamePak = 1 << 13,
}

impl InterruptSource {
    pub const fn mask(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptController {
    interrupt_enable: u16,
    interrupt_flags: u16,
    master_enable: bool,
}

impl InterruptController {
    pub const SUPPORTED_MASK: u16 = 0x3FFF;

    pub const fn new() -> Self {
        Self {
            interrupt_enable: 0,
            interrupt_flags: 0,
            master_enable: false,
        }
    }

    pub const fn interrupt_enable(&self) -> u16 {
        self.interrupt_enable
    }

    pub fn set_interrupt_enable(&mut self, value: u16) {
        self.interrupt_enable = value & Self::SUPPORTED_MASK;
    }

    pub const fn interrupt_flags(&self) -> u16 {
        self.interrupt_flags
    }

    pub const fn master_enable(&self) -> bool {
        self.master_enable
    }

    pub fn set_master_enable(&mut self, value: u16) {
        self.master_enable = value & 1 != 0;
    }

    pub fn request(&mut self, source: InterruptSource) {
        self.request_mask(source.mask());
    }

    pub fn request_mask(&mut self, mask: u16) {
        self.interrupt_flags |= mask & Self::SUPPORTED_MASK;
    }

    /// GBA IF semantics:
    ///
    /// Writing a one clears the corresponding flag.
    /// Writing a zero leaves that flag unchanged.
    pub fn acknowledge(&mut self, mask: u16) {
        self.interrupt_flags &= !(mask & Self::SUPPORTED_MASK);
    }

    pub const fn enabled_pending(&self) -> u16 {
        self.interrupt_enable & self.interrupt_flags & Self::SUPPORTED_MASK
    }

    pub const fn irq_line(&self) -> bool {
        self.master_enable && self.enabled_pending() != 0
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{InterruptController, InterruptSource};

    #[test]
    fn starts_disabled() {
        let controller = InterruptController::new();

        assert_eq!(controller.interrupt_enable(), 0);

        assert_eq!(controller.interrupt_flags(), 0);

        assert!(!controller.master_enable());
        assert!(!controller.irq_line());
    }

    #[test]
    fn request_sets_interrupt_flag() {
        let mut controller = InterruptController::new();

        controller.request(InterruptSource::Timer0);

        assert_eq!(controller.interrupt_flags(), InterruptSource::Timer0.mask());
    }

    #[test]
    fn irq_requires_matching_ie_if_and_ime() {
        let mut controller = InterruptController::new();

        controller.request(InterruptSource::Timer0);

        assert!(!controller.irq_line());

        controller.set_interrupt_enable(InterruptSource::Timer0.mask());

        assert!(!controller.irq_line());

        controller.set_master_enable(1);

        assert!(controller.irq_line());
    }

    #[test]
    fn unrelated_enabled_source_does_not_raise_irq() {
        let mut controller = InterruptController::new();

        controller.request(InterruptSource::Timer0);

        controller.set_interrupt_enable(InterruptSource::VBlank.mask());

        controller.set_master_enable(1);

        assert!(!controller.irq_line());
    }

    #[test]
    fn acknowledge_is_write_one_to_clear() {
        let mut controller = InterruptController::new();

        controller.request(InterruptSource::VBlank);

        controller.request(InterruptSource::Timer0);

        controller.acknowledge(InterruptSource::VBlank.mask());

        assert_eq!(controller.interrupt_flags(), InterruptSource::Timer0.mask());
    }

    #[test]
    fn writing_zero_clears_nothing() {
        let mut controller = InterruptController::new();

        controller.request(InterruptSource::Timer0);

        controller.acknowledge(0);

        assert_eq!(controller.interrupt_flags(), InterruptSource::Timer0.mask());
    }

    #[test]
    fn ime_uses_only_bit_zero() {
        let mut controller = InterruptController::new();

        controller.set_master_enable(0xFFFE);

        assert!(!controller.master_enable());

        controller.set_master_enable(0xFFFF);

        assert!(controller.master_enable());
    }

    #[test]
    fn unsupported_bits_are_masked() {
        let mut controller = InterruptController::new();

        controller.set_interrupt_enable(0xFFFF);

        controller.request_mask(0xFFFF);

        assert_eq!(
            controller.interrupt_enable(),
            InterruptController::SUPPORTED_MASK
        );

        assert_eq!(
            controller.interrupt_flags(),
            InterruptController::SUPPORTED_MASK
        );
    }
}
