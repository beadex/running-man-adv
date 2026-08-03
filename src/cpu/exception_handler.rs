use super::{CpuMode, Exception, InvalidCpuMode, Registers, SpsrAccessError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionEntryResult {
    pub exception: Exception,
    pub previous_mode: CpuMode,
    pub new_mode: CpuMode,
    pub return_address: u32,
    pub vector: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionReturnResult {
    pub previous_mode: CpuMode,
    pub restored_mode: CpuMode,
    pub target: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionError {
    SpsrUnavailable(SpsrAccessError),
    InvalidRestoredMode(InvalidCpuMode),
}

pub fn enter_exception(
    registers: &mut Registers,
    exception: Exception,
    return_address: u32,
) -> Result<ExceptionEntryResult, ExceptionError> {
    /*
     * Snapshot CPSR before switching mode.
     */
    let old_cpsr = registers.cpsr();

    let previous_mode = old_cpsr
        .mode()
        .expect("current CPSR must contain a valid CPU mode");

    let new_mode = exception.mode();

    /*
     * Switch mode first so set_spsr() and LR access select the
     * destination exception bank.
     */
    registers.cpsr_mut().set_mode(new_mode);

    /*
     * SPSR_<exception mode> receives the complete old CPSR.
     */
    registers
        .set_spsr(old_cpsr)
        .map_err(ExceptionError::SpsrUnavailable)?;

    /*
     * LR_<exception mode> receives the exception-specific return
     * address supplied by the caller.
     */
    registers.write(Registers::LR, return_address);

    /*
     * Exceptions always enter ARM state.
     */
    registers.cpsr_mut().set_thumb_state(false);

    if exception.disables_irq() {
        registers.cpsr_mut().set_irq_disabled(true);
    }

    if exception.disables_fiq() {
        registers.cpsr_mut().set_fiq_disabled(true);
    }

    let vector = exception.vector();

    registers.set_pc(vector);

    Ok(ExceptionEntryResult {
        exception,
        previous_mode,
        new_mode,
        return_address,
        vector,
    })
}

pub fn return_from_exception(
    registers: &mut Registers,
    raw_target: u32,
) -> Result<ExceptionReturnResult, ExceptionError> {
    let previous_mode = registers.mode();

    /*
     * Must read the current exception mode's SPSR before CPSR is
     * restored, because restoring CPSR can switch register banks.
     */
    let restored_cpsr = registers.spsr().map_err(ExceptionError::SpsrUnavailable)?;

    let restored_mode = restored_cpsr
        .mode()
        .map_err(ExceptionError::InvalidRestoredMode)?;

    let target = if restored_cpsr.thumb_state() {
        raw_target & !1
    } else {
        raw_target & !3
    };

    registers.set_cpsr_raw(restored_cpsr.raw());

    registers.set_pc(target);

    Ok(ExceptionReturnResult {
        previous_mode,
        restored_mode,
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::{enter_exception, return_from_exception};

    use crate::cpu::{CpuMode, Exception, Registers};

    #[test]
    fn software_interrupt_enters_supervisor_mode() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_thumb_state(true);

        registers.cpsr_mut().set_zero(true);

        let old_cpsr = registers.cpsr();

        let result =
            enter_exception(&mut registers, Exception::SoftwareInterrupt, 0x0800_0102).unwrap();

        assert_eq!(result.previous_mode, CpuMode::System);

        assert_eq!(registers.mode(), CpuMode::Supervisor);

        assert_eq!(registers.spsr().unwrap(), old_cpsr);

        assert_eq!(registers.read(Registers::LR), 0x0800_0102);

        assert_eq!(registers.pc(), 0x0000_0008);

        assert!(registers.cpsr().irq_disabled());

        assert!(!registers.cpsr().thumb_state());
    }

    #[test]
    fn exception_entry_uses_destination_banked_lr() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.write(Registers::LR, 0x1111_1111);

        enter_exception(&mut registers, Exception::Irq, 0x0800_0104).unwrap();

        assert_eq!(registers.mode(), CpuMode::Irq);

        assert_eq!(registers.read(Registers::LR), 0x0800_0104);

        registers.cpsr_mut().set_mode(CpuMode::System);

        assert_eq!(registers.read(Registers::LR), 0x1111_1111);
    }

    #[test]
    fn exception_return_restores_arm_state() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_thumb_state(false);

        let original_cpsr = registers.cpsr();

        enter_exception(&mut registers, Exception::SoftwareInterrupt, 0x0800_0100).unwrap();

        let result = return_from_exception(&mut registers, 0x0800_0103).unwrap();

        assert_eq!(result.restored_mode, CpuMode::System);

        assert_eq!(registers.cpsr(), original_cpsr);

        assert_eq!(registers.pc(), 0x0800_0100);
    }

    #[test]
    fn exception_return_restores_thumb_state() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_thumb_state(true);

        enter_exception(&mut registers, Exception::SoftwareInterrupt, 0x0800_0102).unwrap();

        assert!(!registers.cpsr().thumb_state());

        return_from_exception(&mut registers, 0x0800_0103).unwrap();

        assert_eq!(registers.mode(), CpuMode::System);

        assert!(registers.cpsr().thumb_state());

        assert_eq!(registers.pc(), 0x0800_0102);
    }

    #[test]
    fn exception_return_restores_flags_and_masks() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_zero(true);

        registers.cpsr_mut().set_carry(true);

        registers.cpsr_mut().set_irq_disabled(false);

        let expected = registers.cpsr();

        enter_exception(&mut registers, Exception::SoftwareInterrupt, 0x0800_0100).unwrap();

        assert!(registers.cpsr().irq_disabled());

        return_from_exception(&mut registers, 0x0800_0100).unwrap();

        assert_eq!(registers.cpsr(), expected);
    }
}
