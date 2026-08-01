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

    pub const fn new() -> Self {
        Self { value: 0 }
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
    use super::Cpsr;

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
        cpsr.set_zero(true);
        cpsr.set_carry(true);
        cpsr.set_overflow(true);

        assert_eq!(cpsr.raw(), 0xF000_0000);

        cpsr.set_negative(false);
        cpsr.set_zero(false);
        cpsr.set_carry(false);
        cpsr.set_overflow(false);

        assert_eq!(cpsr.raw(), 0);
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

        cpsr.set_nzcv(true, false, true, false);

        assert!(cpsr.negative());
        assert!(!cpsr.zero());
        assert!(cpsr.carry());
        assert!(!cpsr.overflow());

        assert_eq!(cpsr.raw(), 0xA000_0000);
    }

    #[test]
    fn reads_flags_from_raw_value() {
        let cpsr = Cpsr::from_raw(0x9000_0000);

        assert!(cpsr.negative());
        assert!(!cpsr.zero());
        assert!(!cpsr.carry());
        assert!(cpsr.overflow());
    }
}
