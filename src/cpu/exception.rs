use super::CpuMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    Reset,
    UndefinedInstruction,
    SoftwareInterrupt,
    PrefetchAbort,
    DataAbort,
    Irq,
    Fiq,
}

impl Exception {
    pub const fn vector(self) -> u32 {
        match self {
            Self::Reset => 0x0000_0000,
            Self::UndefinedInstruction => 0x0000_0004,
            Self::SoftwareInterrupt => 0x0000_0008,
            Self::PrefetchAbort => 0x0000_000C,
            Self::DataAbort => 0x0000_0010,
            Self::Irq => 0x0000_0018,
            Self::Fiq => 0x0000_001C,
        }
    }

    pub const fn mode(self) -> CpuMode {
        match self {
            Self::Reset | Self::SoftwareInterrupt => CpuMode::Supervisor,

            Self::UndefinedInstruction => CpuMode::Undefined,

            Self::PrefetchAbort | Self::DataAbort => CpuMode::Abort,

            Self::Irq => CpuMode::Irq,
            Self::Fiq => CpuMode::Fiq,
        }
    }

    pub const fn disables_irq(self) -> bool {
        /*
         * Every ARM7TDMI exception entry disables IRQ.
         */
        true
    }

    pub const fn disables_fiq(self) -> bool {
        /*
         * FIQ and reset disable FIQ.
         *
         * Other exceptions preserve CPSR.F.
         */
        matches!(self, Self::Reset | Self::Fiq)
    }
}
