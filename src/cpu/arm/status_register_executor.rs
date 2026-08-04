use crate::cpu::{Cpsr, CpuMode, Registers, SpsrAccessError};

use super::{
    ProgramStatusRegister, StatusRegisterInstruction, StatusRegisterMask, StatusRegisterOperand,
    expand_rotated_immediate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusRegisterExecutionError {
    SpsrUnavailable(SpsrAccessError),
    InvalidMode(u8),
}

pub fn execute_status_register(
    registers: &mut Registers,
    instruction: StatusRegisterInstruction,
) -> Result<(), StatusRegisterExecutionError> {
    match instruction {
        StatusRegisterInstruction::Mrs { source, rd, .. } => execute_mrs(registers, source, rd),

        StatusRegisterInstruction::Msr {
            destination,
            mask,
            operand,
            ..
        } => execute_msr(registers, destination, mask, operand),
    }
}

fn execute_mrs(
    registers: &mut Registers,
    source: ProgramStatusRegister,
    rd: u8,
) -> Result<(), StatusRegisterExecutionError> {
    let value = match source {
        ProgramStatusRegister::Cpsr => registers.cpsr().raw(),

        ProgramStatusRegister::Spsr => registers
            .spsr()
            .map_err(StatusRegisterExecutionError::SpsrUnavailable)?
            .raw(),
    };

    registers.write(rd as usize, value);

    Ok(())
}

fn evaluate_operand(registers: &Registers, operand: StatusRegisterOperand) -> u32 {
    match operand {
        StatusRegisterOperand::Register(rm) => registers.read(rm as usize),

        StatusRegisterOperand::Immediate { value, rotate } => {
            expand_rotated_immediate(value, rotate, registers.cpsr().carry()).value
        }
    }
}

fn execute_msr(
    registers: &mut Registers,
    destination: ProgramStatusRegister,
    fields: StatusRegisterMask,
    operand: StatusRegisterOperand,
) -> Result<(), StatusRegisterExecutionError> {
    let operand = evaluate_operand(registers, operand);

    match destination {
        ProgramStatusRegister::Cpsr => write_cpsr(registers, fields, operand),

        ProgramStatusRegister::Spsr => write_spsr(registers, fields, operand),
    }
}

fn write_cpsr(
    registers: &mut Registers,
    fields: StatusRegisterMask,
    operand: u32,
) -> Result<(), StatusRegisterExecutionError> {
    let current = registers.cpsr();
    let current_mode = registers.mode();

    let mut writable_mask = fields.bit_mask();

    if !current_mode.is_privileged() {
        /*
         * User mode may only modify the flags byte.
         */
        writable_mask &= 0xFF00_0000;
    }

    /*
     * MSR must not be used to switch instruction-set state.
     * Preserve CPSR.T.
     */
    writable_mask &= !Cpsr::THUMB_STATE_MASK;

    let candidate = (current.raw() & !writable_mask) | (operand & writable_mask);

    /*
     * Validate mode only when mode bits are actually writable.
     */
    if writable_mask & Cpsr::MODE_MASK != 0 {
        let mode_bits = (candidate & Cpsr::MODE_MASK) as u8;

        CpuMode::from_bits(mode_bits)
            .map_err(|_| StatusRegisterExecutionError::InvalidMode(mode_bits))?;
    }

    registers.set_cpsr_raw(candidate);

    Ok(())
}

fn write_spsr(
    registers: &mut Registers,
    fields: StatusRegisterMask,
    operand: u32,
) -> Result<(), StatusRegisterExecutionError> {
    let current = registers
        .spsr()
        .map_err(StatusRegisterExecutionError::SpsrUnavailable)?;

    let mask = fields.bit_mask();

    let candidate = (current.raw() & !mask) | (operand & mask);

    registers
        .set_spsr_raw(candidate)
        .map_err(StatusRegisterExecutionError::SpsrUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cpu::arm::ArmCondition;
    use crate::cpu::{CpuMode, Registers};

    #[test]
    fn mrs_reads_cpsr() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_zero(true);

        execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Mrs {
                condition: ArmCondition::Always,
                source: ProgramStatusRegister::Cpsr,
                rd: 0,
            },
        )
        .unwrap();

        assert_eq!(registers.read(0), registers.cpsr().raw());
    }

    #[test]
    fn mrs_reads_current_modes_spsr() {
        let mut registers = Registers::new();

        registers.set_spsr_raw(0x6000_0010).unwrap();

        execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Mrs {
                condition: ArmCondition::Always,
                source: ProgramStatusRegister::Spsr,
                rd: 1,
            },
        )
        .unwrap();

        assert_eq!(registers.read(1), 0x6000_0010);
    }

    #[test]
    fn user_mode_can_write_flags_but_not_control() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::User);

        registers.write(0, 0xF000_0013);

        execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Msr {
                condition: ArmCondition::Always,

                destination: ProgramStatusRegister::Cpsr,

                mask: StatusRegisterMask {
                    control: true,
                    extension: false,
                    status: false,
                    flags: true,
                },

                operand: StatusRegisterOperand::Register(0),
            },
        )
        .unwrap();

        assert_eq!(registers.mode(), CpuMode::User);

        assert_eq!(registers.cpsr().raw() & 0xF000_0000, 0xF000_0000);
    }

    #[test]
    fn privileged_msr_can_switch_mode() {
        let mut registers = Registers::new();

        registers.write(Registers::SP, 0x0300_7FE0);

        registers.cpsr_mut().set_mode(CpuMode::Irq);

        registers.write(Registers::SP, 0x0300_7FA0);

        registers.cpsr_mut().set_mode(CpuMode::Supervisor);

        registers.write(0, CpuMode::Irq as u32);

        execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Msr {
                condition: ArmCondition::Always,

                destination: ProgramStatusRegister::Cpsr,

                mask: StatusRegisterMask {
                    control: true,
                    extension: false,
                    status: false,
                    flags: false,
                },

                operand: StatusRegisterOperand::Register(0),
            },
        )
        .unwrap();

        assert_eq!(registers.mode(), CpuMode::Irq);

        assert_eq!(registers.read(Registers::SP), 0x0300_7FA0);
    }

    #[test]
    fn msr_preserves_thumb_bit() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_thumb_state(false);

        registers.write(0, Cpsr::THUMB_STATE_MASK | CpuMode::Supervisor as u32);

        execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Msr {
                condition: ArmCondition::Always,
                destination: ProgramStatusRegister::Cpsr,

                mask: StatusRegisterMask {
                    control: true,
                    extension: false,
                    status: false,
                    flags: false,
                },

                operand: StatusRegisterOperand::Register(0),
            },
        )
        .unwrap();

        assert!(!registers.cpsr().thumb_state());
    }

    #[test]
    fn system_mode_cannot_access_spsr() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        let result = execute_status_register(
            &mut registers,
            StatusRegisterInstruction::Mrs {
                condition: ArmCondition::Always,
                source: ProgramStatusRegister::Spsr,
                rd: 0,
            },
        );

        assert!(matches!(
            result,
            Err(StatusRegisterExecutionError::SpsrUnavailable(_))
        ));
    }
}
