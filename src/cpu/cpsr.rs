use super::{CpuMode, InvalidCpuMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cpsr {
    value: u32,
}

impl Cpsr {
    pub const NEGATIVE_BIT: u32 = 31;
    pub const ZERO_BIT: u32 = 30;
    pub const CARRY_BIT: u32 = 29;
    pub const OVERFLOW_BIT: u32 = 28;
    pub const NEGATIVE_MASK: u32 = 1 << Self::NEGATIVE_BIT;
    pub const ZERO_MASK: u32 = 1 << Self::ZERO_BIT;
    pub const CARRY_MASK: u32 = 1 << Self::CARRY_BIT;
    pub const OVERFLOW_MASK: u32 = 1 << Self::OVERFLOW_BIT;

    pub const CONDITION_FLAGS_MASK: u32 =
        Self::NEGATIVE_MASK | Self::ZERO_MASK | Self::CARRY_MASK | Self::OVERFLOW_MASK;

    pub const IRQ_DISABLE_BIT: u32 = 7;
    pub const FIQ_DISABLE_BIT: u32 = 6;
    pub const THUMB_STATE_BIT: u32 = 5;

    pub const IRQ_DISABLE_MASK: u32 = 1 << Self::IRQ_DISABLE_BIT;
    pub const FIQ_DISABLE_MASK: u32 = 1 << Self::FIQ_DISABLE_BIT;
    pub const THUMB_STATE_MASK: u32 = 1 << Self::THUMB_STATE_BIT;

    pub const MODE_MASK: u32 = 0x1F;

    pub const fn new() -> Self {
        Self {
            value: CpuMode::Supervisor as u32 | Self::IRQ_DISABLE_MASK | Self::FIQ_DISABLE_MASK,
        }
    }

    pub const fn from_raw(value: u32) -> Self {
        Self { value }
    }

    pub const fn raw(self) -> u32 {
        self.value
    }

    pub fn set_raw(&mut self, value: u32) {
        self.value = value;
    }

    pub const fn mode(self) -> Result<CpuMode, InvalidCpuMode> {
        CpuMode::from_bits((self.value & Self::MODE_MASK) as u8)
    }

    pub const fn negative(self) -> bool {
        self.value & Self::NEGATIVE_MASK != 0
    }

    pub const fn zero(self) -> bool {
        self.value & Self::ZERO_MASK != 0
    }

    pub const fn carry(self) -> bool {
        self.value & Self::CARRY_MASK != 0
    }

    pub const fn overflow(self) -> bool {
        self.value & Self::OVERFLOW_MASK != 0
    }

    pub const fn irq_disabled(self) -> bool {
        self.value & Self::IRQ_DISABLE_MASK != 0
    }

    pub const fn fiq_disabled(self) -> bool {
        self.value & Self::FIQ_DISABLE_MASK != 0
    }

    pub const fn thumb_state(self) -> bool {
        self.value & Self::THUMB_STATE_MASK != 0
    }

    pub fn set_mode(&mut self, mode: CpuMode) {
        self.value = (self.value & !Self::MODE_MASK) | mode as u32;
    }

    pub fn set_negative(&mut self, value: bool) {
        self.set_flag(Self::NEGATIVE_MASK, value);
    }

    pub fn set_zero(&mut self, value: bool) {
        self.set_flag(Self::ZERO_MASK, value);
    }

    pub fn set_carry(&mut self, value: bool) {
        self.set_flag(Self::CARRY_MASK, value);
    }

    pub fn set_overflow(&mut self, value: bool) {
        self.set_flag(Self::OVERFLOW_MASK, value);
    }

    pub fn set_irq_disabled(&mut self, disabled: bool) {
        self.set_flag(Self::IRQ_DISABLE_MASK, disabled);
    }

    pub fn set_fiq_disabled(&mut self, disabled: bool) {
        self.set_flag(Self::FIQ_DISABLE_MASK, disabled);
    }

    pub fn set_thumb_state(&mut self, enabled: bool) {
        self.set_flag(Self::THUMB_STATE_MASK, enabled);
    }

    pub fn set_nzcv(&mut self, negative: bool, zero: bool, carry: bool, overflow: bool) {
        self.set_negative(negative);
        self.set_zero(zero);
        self.set_carry(carry);
        self.set_overflow(overflow);
    }

    fn set_flag(&mut self, mask: u32, enabled: bool) {
        if enabled {
            self.value |= mask;
        } else {
            self.value &= !mask;
        }
    }
}

impl Default for Cpsr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Cpsr, CpuMode};

    #[test]
    fn new_cpsr_starts_in_supervisor_mode() {
        let cpsr = Cpsr::new();

        assert_eq!(cpsr.mode(), Ok(CpuMode::Supervisor));

        assert!(cpsr.irq_disabled());
        assert!(cpsr.fiq_disabled());
        assert!(!cpsr.thumb_state());
    }

    #[test]
    fn flags_are_clear_by_default() {
        let cpsr = Cpsr::new();

        assert!(!cpsr.negative());
        assert!(!cpsr.zero());
        assert!(!cpsr.carry());
        assert!(!cpsr.overflow());
    }

    #[test]
    fn can_set_and_clear_individual_flags() {
        let mut cpsr = Cpsr::new();

        cpsr.set_negative(true);

        assert!(cpsr.negative());
        assert_eq!(cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK, Cpsr::NEGATIVE_MASK,);

        cpsr.set_zero(true);

        assert!(cpsr.zero());
        assert_eq!(
            cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK,
            Cpsr::NEGATIVE_MASK | Cpsr::ZERO_MASK,
        );

        cpsr.set_carry(true);

        assert!(cpsr.carry());
        assert_eq!(
            cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK,
            Cpsr::NEGATIVE_MASK | Cpsr::ZERO_MASK | Cpsr::CARRY_MASK,
        );

        cpsr.set_overflow(true);

        assert!(cpsr.overflow());
        assert_eq!(
            cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK,
            Cpsr::CONDITION_FLAGS_MASK,
        );

        cpsr.set_negative(false);
        cpsr.set_zero(false);
        cpsr.set_carry(false);
        cpsr.set_overflow(false);

        assert!(!cpsr.negative());
        assert!(!cpsr.zero());
        assert!(!cpsr.carry());
        assert!(!cpsr.overflow());

        assert_eq!(cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK, 0,);

        /*
         * Clearing condition flags must not destroy control-state bits.
         */
        assert_eq!(cpsr.mode(), Ok(CpuMode::Supervisor),);

        assert!(cpsr.irq_disabled());
        assert!(cpsr.fiq_disabled());
    }

    #[test]
    fn changing_flags_preserves_other_bits() {
        let mut cpsr = Cpsr::from_raw(0x0000_001F);

        cpsr.set_zero(true);

        assert_eq!(cpsr.raw(), 0x4000_001F);

        cpsr.set_zero(false);

        assert_eq!(cpsr.raw(), 0x0000_001F);
    }

    #[test]
    fn can_set_all_condition_flags_together() {
        let mut cpsr = Cpsr::new();

        let control_bits = cpsr.raw() & !Cpsr::CONDITION_FLAGS_MASK;

        cpsr.set_nzcv(true, false, true, false);

        assert_eq!(
            cpsr.raw() & Cpsr::CONDITION_FLAGS_MASK,
            Cpsr::NEGATIVE_MASK | Cpsr::CARRY_MASK,
        );

        assert_eq!(cpsr.raw() & !Cpsr::CONDITION_FLAGS_MASK, control_bits,);
    }

    #[test]
    fn reads_flags_from_raw_value() {
        let cpsr = Cpsr::from_raw(0x9000_0000);

        assert!(cpsr.negative());
        assert!(!cpsr.zero());
        assert!(!cpsr.carry());
        assert!(cpsr.overflow());
    }

    #[test]
    fn can_set_thumb_state_bit() {
        let mut cpsr = Cpsr::new();

        assert!(!cpsr.thumb_state());

        cpsr.set_thumb_state(true);

        assert!(cpsr.thumb_state());
        assert_eq!(cpsr.raw() & Cpsr::THUMB_STATE_MASK, Cpsr::THUMB_STATE_MASK);

        cpsr.set_thumb_state(false);

        assert!(!cpsr.thumb_state());
    }
}
