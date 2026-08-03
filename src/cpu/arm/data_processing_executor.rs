use crate::cpu::{ExceptionError, Registers, return_from_exception};

use super::{
    AluFlags, DataProcessingInstruction, DataProcessingOpcode, Operand2, ShiftAmount, ShiftResult,
    add_with_carry, expand_rotated_immediate, logical_flags, shift_immediate, shift_register,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProcessingExecutionError {
    ExceptionReturn(ExceptionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataProcessingExecutionResult {
    pub cycles: u32,
    pub branch: bool,
}

impl DataProcessingExecutionResult {
    pub const fn sequential(cycles: u32) -> Self {
        Self {
            cycles,
            branch: false,
        }
    }

    pub const fn branched(cycles: u32) -> Self {
        Self {
            cycles,
            branch: true,
        }
    }
}

pub fn execute_data_processing(
    registers: &mut Registers,
    instruction: DataProcessingInstruction,
    instruction_address: u32,
) -> Result<DataProcessingExecutionResult, DataProcessingExecutionError> {
    let old_cpsr = registers.cpsr();
    let old_carry = old_cpsr.carry();

    let operand2 = evaluate_operand2(
        registers,
        instruction.operand2,
        old_carry,
        instruction_address,
    );

    let rn_value = if instruction.opcode.uses_rn() {
        read_arm_operand_register(registers, instruction.rn, instruction_address)
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
            if instruction.set_flags {
                /*
                 * Data-processing instruction with S=1 and Rd=PC:
                 *
                 * CPSR <- SPSR
                 * PC   <- ALU result
                 */
                return_from_exception(registers, outcome.value)
                    .map_err(DataProcessingExecutionError::ExceptionReturn)?;

                return Ok(DataProcessingExecutionResult::branched(1));
            }

            /*
             * Ordinary data-processing write to PC.
             *
             * It remains in the current instruction-set state.
             */
            let target = if registers.cpsr().thumb_state() {
                outcome.value & !1
            } else {
                outcome.value & !3
            };

            registers.set_pc(target);

            return Ok(DataProcessingExecutionResult::branched(1));
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

    Ok(DataProcessingExecutionResult::sequential(1))
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
    instruction_address: u32,
) -> ShiftResult {
    match operand2 {
        Operand2::Immediate { value, rotate } => expand_rotated_immediate(value, rotate, old_carry),

        Operand2::Register(shift) => match shift.amount {
            ShiftAmount::Immediate(amount) => {
                /*
                 * For an immediate shift, R15 as Rm reads the normal
                 * ARM architectural PC: instruction address + 8.
                 */
                let value = read_arm_operand_register(registers, shift.rm, instruction_address);

                shift_immediate(value, shift.shift_type, amount, old_carry)
            }

            ShiftAmount::Register(rs) => {
                /*
                 * A register-controlled shift takes an extra internal
                 * cycle. When R15 is Rm, the value observed is PC + 12.
                 *
                 * R15 as Rs is architecturally unusual/unpredictable on
                 * ARM7TDMI, but using the normal PC + 8 operand value is
                 * deterministic and matches the visible register model.
                 */
                let value = read_arm_shifted_rm(registers, shift.rm, instruction_address, true);

                let rs_value = read_arm_operand_register(registers, rs, instruction_address);

                shift_register(value, shift.shift_type, rs_value, old_carry)
            }
        },
    }
}

fn read_arm_operand_register(registers: &Registers, index: u8, instruction_address: u32) -> u32 {
    if index as usize == Registers::PC {
        instruction_address.wrapping_add(8)
    } else {
        registers.read(index as usize)
    }
}

fn read_arm_shifted_rm(
    registers: &Registers,
    index: u8,
    instruction_address: u32,
    shift_by_register: bool,
) -> u32 {
    if index as usize == Registers::PC {
        instruction_address.wrapping_add(if shift_by_register { 12 } else { 8 })
    } else {
        registers.read(index as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::execute_data_processing;

    use crate::cpu::{
        CpuMode, Registers,
        arm::{DataProcessingOpcode, decode_data_processing},
    };

    const TEST_INSTRUCTION_ADDRESS: u32 = 0x0200_0000;

    fn execute(registers: &mut Registers, raw_instruction: u32) {
        let instruction = decode_data_processing(raw_instruction).unwrap();

        execute_data_processing(registers, instruction, TEST_INSTRUCTION_ADDRESS).unwrap();
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

    #[test]
    fn movs_pc_lr_returns_from_exception() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_thumb_state(true);

        registers.cpsr_mut().set_zero(true);

        let original_cpsr = registers.cpsr();

        crate::cpu::enter_exception(
            &mut registers,
            crate::cpu::Exception::SoftwareInterrupt,
            0x0800_0102,
        )
        .unwrap();

        assert_eq!(registers.mode(), CpuMode::Supervisor);

        /*
         * MOVS PC, LR
         */
        let instruction = decode_data_processing(0xE1B0_F00E).unwrap();

        execute_data_processing(&mut registers, instruction, 0x0000_0008).unwrap();

        assert_eq!(registers.cpsr(), original_cpsr);

        assert_eq!(registers.pc(), 0x0800_0102);

        assert_eq!(registers.mode(), CpuMode::System);

        assert!(registers.cpsr().thumb_state());
    }
    #[test]
    fn add_can_read_pc_as_rn() {
        let mut registers = Registers::new();

        /*
         * ADD R0, PC, #1
         *
         * At 0x00000114, ARM architectural PC is 0x0000011C.
         */
        let instruction = decode_data_processing(0xE28F_0001).unwrap();

        let result = execute_data_processing(&mut registers, instruction, 0x0000_0114).unwrap();

        assert_eq!(registers.read(0), 0x0000_011D);
        assert!(!result.branch);
    }

    #[test]
    fn immediate_shift_can_read_pc_as_rm() {
        let mut registers = Registers::new();

        /*
         * MOV R0, PC
         */
        let instruction = decode_data_processing(0xE1A0_000F).unwrap();

        execute_data_processing(&mut registers, instruction, 0x0200_0000).unwrap();

        assert_eq!(registers.read(0), 0x0200_0008);
    }

    #[test]
    fn register_shift_reads_pc_as_rm_plus_twelve() {
        let mut registers = Registers::new();
        registers.write(1, 0);

        /*
         * MOV R0, PC, LSL R1
         *
         * With a register-controlled shift, Rm=PC observes PC+12.
         */
        let instruction = decode_data_processing(0xE1A0_011F).unwrap();

        execute_data_processing(&mut registers, instruction, 0x0200_0000).unwrap();

        assert_eq!(registers.read(0), 0x0200_000C);
    }

    #[test]
    fn ordinary_write_to_pc_reports_branch() {
        let mut registers = Registers::new();
        registers.write(0, 0x0800_0123);

        /*
         * MOV PC, R0
         */
        let instruction = decode_data_processing(0xE1A0_F000).unwrap();

        let result = execute_data_processing(&mut registers, instruction, 0x0200_0000).unwrap();

        assert!(result.branch);
        assert_eq!(registers.pc(), 0x0800_0120);
    }
}
