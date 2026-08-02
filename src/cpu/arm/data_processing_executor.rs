use crate::cpu::Registers;

use super::{
    AluFlags, DataProcessingInstruction, DataProcessingOpcode, Operand2, ShiftAmount, ShiftResult,
    add_with_carry, expand_rotated_immediate, logical_flags, shift_immediate, shift_register,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProcessingExecutionError {
    /*
     * Writing R15 has additional branch, pipeline refill, alignment,
     * and CPSR/SPSR behavior. It is deferred until the pipeline and
     * exception model exist.
     */
    DestinationIsProgramCounter,

    /*
     * Reading R15 as Operand2 or shift register requires
     * pipeline-visible PC semantics.
     */
    OperandUsesProgramCounter,
}

pub fn execute_data_processing(
    registers: &mut Registers,
    instruction: DataProcessingInstruction,
) -> Result<(), DataProcessingExecutionError> {
    let old_cpsr = registers.cpsr();
    let old_carry = old_cpsr.carry();

    let operand2 = evaluate_operand2(registers, instruction.operand2, old_carry)?;

    let rn_value = if instruction.opcode.uses_rn() {
        read_operand_register(registers, instruction.rn)?
    } else {
        0
    };

    let outcome = execute_operation(
        instruction.opcode,
        rn_value,
        operand2,
        old_carry,
        old_cpsr.overflow(),
    );

    if instruction.opcode.writes_result() {
        if instruction.rd as usize == Registers::PC {
            return Err(DataProcessingExecutionError::DestinationIsProgramCounter);
        }

        registers.write(instruction.rd as usize, outcome.value);
    }

    /*
     * TST, TEQ, CMP and CMN always update flags.
     *
     * Other data-processing instructions update flags only when S=1.
     */
    if instruction.set_flags || instruction.opcode.is_test() {
        let flags = outcome.flags;

        registers
            .cpsr_mut()
            .set_nzcv(flags.negative, flags.zero, flags.carry, flags.overflow);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct OperationOutcome {
    value: u32,
    flags: AluFlags,
}

fn execute_operation(
    opcode: DataProcessingOpcode,
    rn: u32,
    operand2: ShiftResult,
    old_carry: bool,
    old_overflow: bool,
) -> OperationOutcome {
    match opcode {
        DataProcessingOpcode::And => {
            logical_outcome(rn & operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Eor => {
            logical_outcome(rn ^ operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Sub => arithmetic_outcome(add_with_carry(rn, !operand2.value, true)),

        DataProcessingOpcode::Rsb => arithmetic_outcome(add_with_carry(operand2.value, !rn, true)),

        DataProcessingOpcode::Add => arithmetic_outcome(add_with_carry(rn, operand2.value, false)),

        DataProcessingOpcode::Adc => {
            arithmetic_outcome(add_with_carry(rn, operand2.value, old_carry))
        }

        DataProcessingOpcode::Sbc => {
            arithmetic_outcome(add_with_carry(rn, !operand2.value, old_carry))
        }

        DataProcessingOpcode::Rsc => {
            arithmetic_outcome(add_with_carry(operand2.value, !rn, old_carry))
        }

        DataProcessingOpcode::Tst => {
            logical_outcome(rn & operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Teq => {
            logical_outcome(rn ^ operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Cmp => arithmetic_outcome(add_with_carry(rn, !operand2.value, true)),

        DataProcessingOpcode::Cmn => arithmetic_outcome(add_with_carry(rn, operand2.value, false)),

        DataProcessingOpcode::Orr => {
            logical_outcome(rn | operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Mov => logical_outcome(operand2.value, operand2.carry, old_overflow),

        DataProcessingOpcode::Bic => {
            logical_outcome(rn & !operand2.value, operand2.carry, old_overflow)
        }

        DataProcessingOpcode::Mvn => logical_outcome(!operand2.value, operand2.carry, old_overflow),
    }
}

fn logical_outcome(value: u32, shifter_carry: bool, old_overflow: bool) -> OperationOutcome {
    OperationOutcome {
        value,
        flags: logical_flags(value, shifter_carry, old_overflow),
    }
}

fn arithmetic_outcome(result: super::AddResult) -> OperationOutcome {
    OperationOutcome {
        value: result.value,
        flags: result.flags,
    }
}

fn evaluate_operand2(
    registers: &Registers,
    operand2: Operand2,
    old_carry: bool,
) -> Result<ShiftResult, DataProcessingExecutionError> {
    match operand2 {
        Operand2::Immediate { value, rotate } => {
            Ok(expand_rotated_immediate(value, rotate, old_carry))
        }

        Operand2::Register(shift) => {
            let value = read_operand_register(registers, shift.rm)?;

            match shift.amount {
                ShiftAmount::Immediate(amount) => {
                    Ok(shift_immediate(value, shift.shift_type, amount, old_carry))
                }

                ShiftAmount::Register(rs) => {
                    let rs_value = read_operand_register(registers, rs)?;

                    Ok(shift_register(value, shift.shift_type, rs_value, old_carry))
                }
            }
        }
    }
}

fn read_operand_register(
    registers: &Registers,
    index: u8,
) -> Result<u32, DataProcessingExecutionError> {
    if index as usize == Registers::PC {
        return Err(DataProcessingExecutionError::OperandUsesProgramCounter);
    }

    Ok(registers.read(index as usize))
}

#[cfg(test)]
mod tests {
    use super::{DataProcessingExecutionError, execute_data_processing};

    use crate::cpu::{
        Registers,
        arm::{DataProcessingOpcode, decode_data_processing},
    };

    fn execute(registers: &mut Registers, raw_instruction: u32) {
        let instruction = decode_data_processing(raw_instruction).unwrap();

        execute_data_processing(registers, instruction).unwrap();
    }

    #[test]
    fn executes_mov_immediate() {
        let mut registers = Registers::new();

        // MOV R0, #1
        execute(&mut registers, 0xE3A0_0001);

        assert_eq!(registers.read(0), 1);
    }

    #[test]
    fn executes_add_register() {
        let mut registers = Registers::new();

        registers.write(1, 10);
        registers.write(2, 20);

        // ADD R0, R1, R2
        execute(&mut registers, 0xE081_0002);

        assert_eq!(registers.read(0), 30);
    }

    #[test]
    fn adds_and_updates_flags() {
        let mut registers = Registers::new();

        registers.write(1, 0xFFFF_FFFF);
        registers.write(2, 1);

        // ADDS R0, R1, R2
        execute(&mut registers, 0xE091_0002);

        assert_eq!(registers.read(0), 0);
        assert!(registers.cpsr().zero());
        assert!(registers.cpsr().carry());
        assert!(!registers.cpsr().negative());
        assert!(!registers.cpsr().overflow());
    }

    #[test]
    fn detects_signed_addition_overflow() {
        let mut registers = Registers::new();

        registers.write(1, 0x7FFF_FFFF);
        registers.write(2, 1);

        // ADDS R0, R1, R2
        execute(&mut registers, 0xE091_0002);

        assert_eq!(registers.read(0), 0x8000_0000);
        assert!(registers.cpsr().negative());
        assert!(registers.cpsr().overflow());
        assert!(!registers.cpsr().carry());
    }

    #[test]
    fn executes_subtraction() {
        let mut registers = Registers::new();

        registers.write(1, 10);
        registers.write(2, 3);

        // SUB R0, R1, R2
        execute(&mut registers, 0xE041_0002);

        assert_eq!(registers.read(0), 7);
    }

    #[test]
    fn cmp_updates_flags_without_writing_destination() {
        let mut registers = Registers::new();

        registers.write(0, 0xCAFE_BABE);
        registers.write(1, 5);
        registers.write(2, 5);

        // CMP R1, R2
        execute(&mut registers, 0xE151_0002);

        assert_eq!(registers.read(0), 0xCAFE_BABE);
        assert!(registers.cpsr().zero());
        assert!(registers.cpsr().carry());
    }

    #[test]
    fn logical_instruction_preserves_overflow() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_overflow(true);
        registers.write(1, 0xFFFF_0000);
        registers.write(2, 0x0F0F_0F0F);

        // ANDS R0, R1, R2
        execute(&mut registers, 0xE011_0002);

        assert_eq!(registers.read(0), 0x0F0F_0000);

        assert!(registers.cpsr().overflow());
    }

    #[test]
    fn movs_uses_barrel_shifter_carry() {
        let mut registers = Registers::new();

        registers.write(1, 0x8000_0000);

        /*
         * MOVS R0, R1, LSL #1
         */
        execute(&mut registers, 0xE1B0_0081);

        assert_eq!(registers.read(0), 0);
        assert!(registers.cpsr().zero());
        assert!(registers.cpsr().carry());
    }

    #[test]
    fn adc_uses_old_carry() {
        let mut registers = Registers::new();

        registers.write(1, 10);
        registers.write(2, 20);
        registers.cpsr_mut().set_carry(true);

        // ADC R0, R1, R2
        execute(&mut registers, 0xE0A1_0002);

        assert_eq!(registers.read(0), 31);
    }

    #[test]
    fn sbc_uses_inverted_borrow_convention() {
        let mut registers = Registers::new();

        registers.write(1, 10);
        registers.write(2, 3);

        /*
         * C=0 means one borrow must be subtracted:
         *
         * 10 - 3 - 1 = 6
         */
        registers.cpsr_mut().set_carry(false);

        // SBC R0, R1, R2
        execute(&mut registers, 0xE0C1_0002);

        assert_eq!(registers.read(0), 6);
    }

    #[test]
    fn rejects_program_counter_destination_for_now() {
        let mut registers = Registers::new();

        // MOV PC, R0
        let instruction = decode_data_processing(0xE1A0_F000).unwrap();

        let result = execute_data_processing(&mut registers, instruction);

        assert_eq!(
            result,
            Err(DataProcessingExecutionError::DestinationIsProgramCounter)
        );
    }

    #[test]
    fn test_opcode_does_not_write_rd() {
        for opcode in [
            DataProcessingOpcode::Tst,
            DataProcessingOpcode::Teq,
            DataProcessingOpcode::Cmp,
            DataProcessingOpcode::Cmn,
        ] {
            assert!(!opcode.writes_result());
        }
    }
}
