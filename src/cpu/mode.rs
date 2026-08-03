#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CpuMode {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCpuMode(pub u8);

impl CpuMode {
    pub const MASK: u32 = 0x1F;

    pub const fn from_bits(bits: u8) -> Result<Self, InvalidCpuMode> {
        match bits & 0x1F {
            0b10000 => Ok(Self::User),
            0b10001 => Ok(Self::Fiq),
            0b10010 => Ok(Self::Irq),
            0b10011 => Ok(Self::Supervisor),
            0b10111 => Ok(Self::Abort),
            0b11011 => Ok(Self::Undefined),
            0b11111 => Ok(Self::System),
            value => Err(InvalidCpuMode(value)),
        }
    }

    pub const fn is_privileged(self) -> bool {
        !matches!(self, Self::User)
    }

    pub const fn has_spsr(self) -> bool {
        matches!(
            self,
            Self::Fiq | Self::Irq | Self::Supervisor | Self::Abort | Self::Undefined
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CpuMode;

    #[test]
    fn decodes_valid_modes() {
        assert_eq!(CpuMode::from_bits(0x10), Ok(CpuMode::User));

        assert_eq!(CpuMode::from_bits(0x13), Ok(CpuMode::Supervisor));

        assert_eq!(CpuMode::from_bits(0x1F), Ok(CpuMode::System));
    }

    #[test]
    fn user_is_not_privileged() {
        assert!(!CpuMode::User.is_privileged());
        assert!(CpuMode::System.is_privileged());
        assert!(CpuMode::Supervisor.is_privileged());
    }

    #[test]
    fn only_exception_modes_have_spsr() {
        assert!(CpuMode::Fiq.has_spsr());
        assert!(CpuMode::Irq.has_spsr());
        assert!(CpuMode::Supervisor.has_spsr());

        assert!(!CpuMode::User.has_spsr());
        assert!(!CpuMode::System.has_spsr());
    }
}
