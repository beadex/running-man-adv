use super::{Cpsr, CpuMode};

#[derive(Debug, Clone, Copy, Default)]
struct RegisterPair {
    r13: u32,
    r14: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpsrAccessError {
    CurrentModeHasNoSpsr(CpuMode),
}

#[derive(Debug, Clone)]
pub struct Registers {
    /*
     * Shared by every mode.
     */
    r0_to_r7: [u32; 8],

    /*
     * Shared by every mode except FIQ.
     */
    non_fiq_r8_to_r12: [u32; 5],

    /*
     * FIQ bank.
     */
    fiq_r8_to_r12: [u32; 5],

    /*
     * User and System share R13/R14.
     */
    user_system: RegisterPair,

    fiq: RegisterPair,
    irq: RegisterPair,
    supervisor: RegisterPair,
    abort: RegisterPair,
    undefined: RegisterPair,

    pc: u32,
    cpsr: Cpsr,

    spsr_fiq: Cpsr,
    spsr_irq: Cpsr,
    spsr_supervisor: Cpsr,
    spsr_abort: Cpsr,
    spsr_undefined: Cpsr,
}

impl Registers {
    pub const SP: usize = 13;
    pub const LR: usize = 14;
    pub const PC: usize = 15;

    pub fn new() -> Self {
        Self {
            r0_to_r7: [0; 8],
            non_fiq_r8_to_r12: [0; 5],
            fiq_r8_to_r12: [0; 5],

            user_system: RegisterPair::default(),
            fiq: RegisterPair::default(),
            irq: RegisterPair::default(),
            supervisor: RegisterPair::default(),
            abort: RegisterPair::default(),
            undefined: RegisterPair::default(),

            pc: 0,
            cpsr: Cpsr::new(),

            spsr_fiq: Cpsr::new(),
            spsr_irq: Cpsr::new(),
            spsr_supervisor: Cpsr::new(),
            spsr_abort: Cpsr::new(),
            spsr_undefined: Cpsr::new(),
        }
    }

    pub fn mode(&self) -> CpuMode {
        self.cpsr
            .mode()
            .expect("CPSR contains invalid processor mode")
    }

    pub fn read(&self, register: usize) -> u32 {
        assert!(register < 16);

        match register {
            0..=7 => self.r0_to_r7[register],

            8..=12 => {
                let index = register - 8;

                if self.mode() == CpuMode::Fiq {
                    self.fiq_r8_to_r12[index]
                } else {
                    self.non_fiq_r8_to_r12[index]
                }
            }

            Self::SP => self.current_pair().r13,
            Self::LR => self.current_pair().r14,
            Self::PC => self.pc,

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, register: usize, value: u32) {
        assert!(register < 16);

        match register {
            0..=7 => {
                self.r0_to_r7[register] = value;
            }

            8..=12 => {
                let index = register - 8;

                if self.mode() == CpuMode::Fiq {
                    self.fiq_r8_to_r12[index] = value;
                } else {
                    self.non_fiq_r8_to_r12[index] = value;
                }
            }

            Self::SP => {
                self.current_pair_mut().r13 = value;
            }

            Self::LR => {
                self.current_pair_mut().r14 = value;
            }

            Self::PC => {
                self.pc = value;
            }

            _ => unreachable!(),
        }
    }

    pub const fn cpsr(&self) -> Cpsr {
        self.cpsr
    }

    pub fn cpsr_mut(&mut self) -> &mut Cpsr {
        &mut self.cpsr
    }

    pub fn set_cpsr_raw(&mut self, value: u32) {
        self.cpsr.set_raw(value);
    }

    pub const fn pc(&self) -> u32 {
        self.pc
    }

    pub fn set_pc(&mut self, value: u32) {
        self.pc = value;
    }

    fn current_pair(&self) -> &RegisterPair {
        match self.mode() {
            CpuMode::User | CpuMode::System => &self.user_system,

            CpuMode::Fiq => &self.fiq,
            CpuMode::Irq => &self.irq,
            CpuMode::Supervisor => &self.supervisor,
            CpuMode::Abort => &self.abort,
            CpuMode::Undefined => &self.undefined,
        }
    }

    fn current_pair_mut(&mut self) -> &mut RegisterPair {
        match self.mode() {
            CpuMode::User | CpuMode::System => &mut self.user_system,

            CpuMode::Fiq => &mut self.fiq,
            CpuMode::Irq => &mut self.irq,
            CpuMode::Supervisor => &mut self.supervisor,

            CpuMode::Abort => &mut self.abort,
            CpuMode::Undefined => &mut self.undefined,
        }
    }

    pub fn spsr(&self) -> Result<Cpsr, SpsrAccessError> {
        match self.mode() {
            CpuMode::Fiq => Ok(self.spsr_fiq),
            CpuMode::Irq => Ok(self.spsr_irq),

            CpuMode::Supervisor => Ok(self.spsr_supervisor),

            CpuMode::Abort => Ok(self.spsr_abort),

            CpuMode::Undefined => Ok(self.spsr_undefined),

            mode @ (CpuMode::User | CpuMode::System) => {
                Err(SpsrAccessError::CurrentModeHasNoSpsr(mode))
            }
        }
    }

    pub fn set_spsr(&mut self, value: Cpsr) -> Result<(), SpsrAccessError> {
        match self.mode() {
            CpuMode::Fiq => self.spsr_fiq = value,
            CpuMode::Irq => self.spsr_irq = value,

            CpuMode::Supervisor => {
                self.spsr_supervisor = value;
            }

            CpuMode::Abort => self.spsr_abort = value,

            CpuMode::Undefined => {
                self.spsr_undefined = value;
            }

            mode @ (CpuMode::User | CpuMode::System) => {
                return Err(SpsrAccessError::CurrentModeHasNoSpsr(mode));
            }
        }

        Ok(())
    }

    pub fn set_spsr_raw(&mut self, value: u32) -> Result<(), SpsrAccessError> {
        self.set_spsr(Cpsr::from_raw(value))
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn supervisor_and_irq_have_separate_stack_pointers() {
    let mut registers = Registers::new();

    assert_eq!(registers.mode(), CpuMode::Supervisor);

    registers.write(Registers::SP, 0x0300_7FE0);

    registers.cpsr_mut().set_mode(CpuMode::Irq);

    registers.write(Registers::SP, 0x0300_7FA0);

    assert_eq!(registers.read(Registers::SP), 0x0300_7FA0);

    registers.cpsr_mut().set_mode(CpuMode::Supervisor);

    assert_eq!(registers.read(Registers::SP), 0x0300_7FE0);
}

#[test]
fn user_and_system_share_sp_and_lr() {
    let mut registers = Registers::new();

    registers.cpsr_mut().set_mode(CpuMode::User);

    registers.write(Registers::SP, 0x0300_7F00);

    registers.cpsr_mut().set_mode(CpuMode::System);

    assert_eq!(registers.read(Registers::SP), 0x0300_7F00);
}

#[test]
fn fiq_has_banked_r8_to_r12() {
    let mut registers = Registers::new();

    registers.cpsr_mut().set_mode(CpuMode::System);

    registers.write(8, 0x1111_1111);
    registers.write(12, 0xCCCC_CCCC);

    registers.cpsr_mut().set_mode(CpuMode::Fiq);

    assert_eq!(registers.read(8), 0);
    assert_eq!(registers.read(12), 0);

    registers.write(8, 0x8888_8888);

    registers.cpsr_mut().set_mode(CpuMode::System);

    assert_eq!(registers.read(8), 0x1111_1111);
}

#[test]
fn exception_modes_have_distinct_spsrs() {
    let mut registers = Registers::new();

    registers.set_spsr_raw(0x1111_1111).unwrap();

    registers.cpsr_mut().set_mode(CpuMode::Irq);

    registers.set_spsr_raw(0x2222_2222).unwrap();

    assert_eq!(registers.spsr().unwrap().raw(), 0x2222_2222);

    registers.cpsr_mut().set_mode(CpuMode::Supervisor);

    assert_eq!(registers.spsr().unwrap().raw(), 0x1111_1111);
}

#[test]
fn user_mode_has_no_spsr() {
    let mut registers = Registers::new();

    registers.cpsr_mut().set_mode(CpuMode::User);

    assert_eq!(
        registers.spsr(),
        Err(SpsrAccessError::CurrentModeHasNoSpsr(CpuMode::User,))
    );
}
