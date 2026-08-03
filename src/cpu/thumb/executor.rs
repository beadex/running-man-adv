use crate::{
    bus::Bus,
    cpu::{Exception, ExceptionError, Registers, enter_exception},
};

use super::{
    ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbCondition, ThumbHighRegisterOperation,
    ThumbImmediateOperation, ThumbInstruction, ThumbShiftOperation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbExecutionResult {
    /*
     * Execution cycles after instruction fetch.
     *
     * Fetch timing is added by Cpu::step_thumb().
     */
    pub cycles: u32,

    /*
     * True when the next instruction fetch must begin a new bus
     * sequence.
     */
    pub branch: bool,
}

impl ThumbExecutionResult {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbExecutionError {
    Exception(ExceptionError),
}

pub fn execute_thumb(
    registers: &mut Registers,
    _bus: &mut Bus,
    instruction: &ThumbInstruction,
    instruction_address: u32,
) -> Result<ThumbExecutionResult, ThumbExecutionError> {
    match instruction {
        ThumbInstruction::MoveShiftedRegister {
            operation,
            offset,
            source,
            destination,
        } => {
            execute_move_shifted_register(registers, *operation, *offset, *source, *destination);

            Ok(ThumbExecutionResult::sequential(1))
        }

        ThumbInstruction::AddSubtract {
            operation,
            operand,
            source,
            destination,
        } => {
            execute_add_subtract(registers, *operation, *operand, *source, *destination);

            Ok(ThumbExecutionResult::sequential(1))
        }

        ThumbInstruction::Immediate {
            operation,
            destination,
            immediate,
        } => {
            execute_immediate(registers, *operation, *destination, *immediate);

            Ok(ThumbExecutionResult::sequential(1))
        }

        ThumbInstruction::HighRegister {
            operation,
            source,
            destination,
        } => {
            let branch = execute_high_register(
                registers,
                *operation,
                *source,
                *destination,
                instruction_address,
            );

            Ok(ThumbExecutionResult { cycles: 1, branch })
        }

        ThumbInstruction::ConditionalBranch { condition, offset } => {
            let taken =
                execute_conditional_branch(registers, *condition, *offset, instruction_address);

            Ok(ThumbExecutionResult {
                cycles: 1,
                branch: taken,
            })
        }

        ThumbInstruction::SoftwareInterrupt { comment: _ } => {
            execute_software_interrupt(registers, instruction_address)
                .map_err(ThumbExecutionError::Exception)?;

            Ok(ThumbExecutionResult::branched(1))
        }

        ThumbInstruction::UnconditionalBranch { offset } => {
            execute_unconditional_branch(registers, *offset, instruction_address);

            Ok(ThumbExecutionResult::branched(1))
        }
    }
}

fn execute_move_shifted_register(
    registers: &mut Registers,
    operation: ThumbShiftOperation,
    offset: u8,
    source: u8,
    destination: u8,
) {
    let value = registers.read(source as usize);

    let old_carry = registers.cpsr().carry();

    let result = match operation {
        ThumbShiftOperation::LogicalLeft => logical_shift_left(value, offset, old_carry),

        ThumbShiftOperation::LogicalRight => logical_shift_right(value, offset),

        ThumbShiftOperation::ArithmeticRight => arithmetic_shift_right(value, offset),
    };

    registers.write(destination as usize, result.value);

    let negative = result.value & 0x8000_0000 != 0;

    let zero = result.value == 0;

    /*
     * Shift instructions update N, Z and C but preserve V.
     */
    let overflow = registers.cpsr().overflow();

    registers
        .cpsr_mut()
        .set_nzcv(negative, zero, result.carry, overflow);
}

fn execute_add_subtract(
    registers: &mut Registers,
    operation: ThumbAddSubtractOperation,

    operand: ThumbAddSubtractOperand,

    source: u8,
    destination: u8,
) {
    let left = registers.read(source as usize);

    let right = match operand {
        ThumbAddSubtractOperand::Register(register) => registers.read(register as usize),

        ThumbAddSubtractOperand::Immediate(value) => value as u32,
    };

    let result = match operation {
        ThumbAddSubtractOperation::Add => add_with_flags(left, right),

        ThumbAddSubtractOperation::Subtract => subtract_with_flags(left, right),
    };

    registers.write(destination as usize, result.value);

    apply_arithmetic_flags(registers, result);
}

fn execute_immediate(
    registers: &mut Registers,
    operation: ThumbImmediateOperation,

    destination: u8,
    immediate: u8,
) {
    let immediate = immediate as u32;

    match operation {
        ThumbImmediateOperation::Move => {
            registers.write(destination as usize, immediate);

            let carry = registers.cpsr().carry();

            let overflow = registers.cpsr().overflow();

            registers.cpsr_mut().set_nzcv(
                immediate & 0x8000_0000 != 0,
                immediate == 0,
                carry,
                overflow,
            );
        }

        ThumbImmediateOperation::Compare => {
            let left = registers.read(destination as usize);

            let result = subtract_with_flags(left, immediate);

            apply_arithmetic_flags(registers, result);
        }

        ThumbImmediateOperation::Add => {
            let left = registers.read(destination as usize);

            let result = add_with_flags(left, immediate);

            registers.write(destination as usize, result.value);

            apply_arithmetic_flags(registers, result);
        }

        ThumbImmediateOperation::Subtract => {
            let left = registers.read(destination as usize);

            let result = subtract_with_flags(left, immediate);

            registers.write(destination as usize, result.value);

            apply_arithmetic_flags(registers, result);
        }
    }
}

fn execute_high_register(
    registers: &mut Registers,
    operation: ThumbHighRegisterOperation,

    source: u8,
    destination: u8,
    instruction_address: u32,
) -> bool {
    /*
     * In THUMB state, reading PC as an operand produces the current
     * instruction address plus four.
     */
    let source_value = read_thumb_register(registers, source, instruction_address);

    match operation {
        ThumbHighRegisterOperation::Add => {
            let destination_value =
                read_thumb_register(registers, destination, instruction_address);

            let value = destination_value.wrapping_add(source_value);

            if destination as usize == Registers::PC {
                /*
                 * ADD PC remains in THUMB state.
                 */
                registers.set_pc(value & !1);

                true
            } else {
                registers.write(destination as usize, value);

                false
            }
        }

        ThumbHighRegisterOperation::Compare => {
            let destination_value =
                read_thumb_register(registers, destination, instruction_address);

            let result = subtract_with_flags(destination_value, source_value);

            apply_arithmetic_flags(registers, result);

            false
        }

        ThumbHighRegisterOperation::Move => {
            if destination as usize == Registers::PC {
                /*
                 * MOV PC remains in THUMB state.
                 */
                registers.set_pc(source_value & !1);

                true
            } else {
                registers.write(destination as usize, source_value);

                false
            }
        }

        ThumbHighRegisterOperation::BranchExchange => {
            let target = source_value;

            let thumb = target & 1 != 0;

            registers.cpsr_mut().set_thumb_state(thumb);

            if thumb {
                registers.set_pc(target & !1);
            } else {
                registers.set_pc(target & !3);
            }

            true
        }
    }
}

fn execute_conditional_branch(
    registers: &mut Registers,
    condition: ThumbCondition,
    offset: i16,
    instruction_address: u32,
) -> bool {
    if !condition_passes(condition, registers) {
        return false;
    }

    /*
     * THUMB branch base is current instruction address + 4.
     */
    let base = instruction_address.wrapping_add(4);

    let target = base.wrapping_add(offset as i32 as u32);

    registers.set_pc(target & !1);

    true
}

fn execute_unconditional_branch(registers: &mut Registers, offset: i32, instruction_address: u32) {
    let base = instruction_address.wrapping_add(4);

    let target = base.wrapping_add(offset as u32);

    registers.set_pc(target & !1);
}

fn execute_software_interrupt(
    registers: &mut Registers,
    instruction_address: u32,
) -> Result<(), ExceptionError> {
    /*
     * THUMB SWI returns to the halfword immediately following the SWI.
     */
    let return_address = instruction_address.wrapping_add(2);

    enter_exception(registers, Exception::SoftwareInterrupt, return_address)?;

    Ok(())
}

fn condition_passes(condition: ThumbCondition, registers: &Registers) -> bool {
    let cpsr = registers.cpsr();

    let negative = cpsr.negative();

    let zero = cpsr.zero();

    let carry = cpsr.carry();

    let overflow = cpsr.overflow();

    match condition {
        ThumbCondition::Equal => zero,

        ThumbCondition::NotEqual => !zero,

        ThumbCondition::CarrySet => carry,

        ThumbCondition::CarryClear => !carry,

        ThumbCondition::Minus => negative,

        ThumbCondition::Plus => !negative,

        ThumbCondition::Overflow => overflow,

        ThumbCondition::NoOverflow => !overflow,

        ThumbCondition::UnsignedHigher => carry && !zero,

        ThumbCondition::UnsignedLowerOrSame => !carry || zero,

        ThumbCondition::SignedGreaterOrEqual => negative == overflow,

        ThumbCondition::SignedLessThan => negative != overflow,

        ThumbCondition::SignedGreaterThan => !zero && negative == overflow,

        ThumbCondition::SignedLessOrEqual => zero || negative != overflow,
    }
}

fn read_thumb_register(registers: &Registers, register: u8, instruction_address: u32) -> u32 {
    if register as usize == Registers::PC {
        instruction_address.wrapping_add(4)
    } else {
        registers.read(register as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShiftResult {
    value: u32,
    carry: bool,
}

fn logical_shift_left(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    if amount == 0 {
        return ShiftResult {
            value,
            carry: old_carry,
        };
    }

    let amount = amount as u32;

    ShiftResult {
        value: value.wrapping_shl(amount),

        carry: value & (1u32 << (32 - amount)) != 0,
    }
}

fn logical_shift_right(value: u32, encoded_amount: u8) -> ShiftResult {
    /*
     * LSR #0 encodes LSR #32.
     */
    let amount = if encoded_amount == 0 {
        32
    } else {
        encoded_amount as u32
    };

    if amount == 32 {
        return ShiftResult {
            value: 0,

            carry: value & 0x8000_0000 != 0,
        };
    }

    ShiftResult {
        value: value >> amount,

        carry: value & (1u32 << (amount - 1)) != 0,
    }
}

fn arithmetic_shift_right(value: u32, encoded_amount: u8) -> ShiftResult {
    /*
     * ASR #0 encodes ASR #32.
     */
    let amount = if encoded_amount == 0 {
        32
    } else {
        encoded_amount as u32
    };

    if amount == 32 {
        let sign = value & 0x8000_0000 != 0;

        return ShiftResult {
            value: if sign { u32::MAX } else { 0 },

            carry: sign,
        };
    }

    ShiftResult {
        value: ((value as i32) >> amount) as u32,

        carry: value & (1u32 << (amount - 1)) != 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArithmeticResult {
    value: u32,
    carry: bool,
    overflow: bool,
}

fn add_with_flags(left: u32, right: u32) -> ArithmeticResult {
    let (value, carry) = left.overflowing_add(right);

    /*
     * Signed overflow occurs when operands have the same sign and the
     * result has the opposite sign.
     */
    let overflow = (!(left ^ right) & (left ^ value) & 0x8000_0000) != 0;

    ArithmeticResult {
        value,
        carry,
        overflow,
    }
}

fn subtract_with_flags(left: u32, right: u32) -> ArithmeticResult {
    let (value, borrow) = left.overflowing_sub(right);

    /*
     * ARM carry after subtraction means "no borrow".
     */
    let carry = !borrow;

    /*
     * Signed overflow occurs when operand signs differ and the result
     * sign differs from the left operand.
     */
    let overflow = ((left ^ right) & (left ^ value) & 0x8000_0000) != 0;

    ArithmeticResult {
        value,
        carry,
        overflow,
    }
}

fn apply_arithmetic_flags(registers: &mut Registers, result: ArithmeticResult) {
    registers.cpsr_mut().set_nzcv(
        result.value & 0x8000_0000 != 0,
        result.value == 0,
        result.carry,
        result.overflow,
    );
}

#[cfg(test)]
mod tests {
    use super::execute_thumb;

    use crate::{
        bus::Bus,
        cpu::{
            CpuMode, Registers,
            thumb::{
                ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbCondition,
                ThumbHighRegisterOperation, ThumbImmediateOperation, ThumbInstruction,
                ThumbShiftOperation,
            },
        },
    };

    fn execute(
        registers: &mut Registers,
        instruction: ThumbInstruction,
        instruction_address: u32,
    ) -> super::ThumbExecutionResult {
        let mut bus = Bus::new();

        execute_thumb(registers, &mut bus, &instruction, instruction_address).unwrap()
    }

    #[test]
    fn lsl_updates_value_and_carry() {
        let mut registers = Registers::new();

        registers.write(0, 0x8000_0001);

        let result = execute(
            &mut registers,
            ThumbInstruction::MoveShiftedRegister {
                operation: ThumbShiftOperation::LogicalLeft,

                offset: 1,
                source: 0,
                destination: 1,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(1), 0x0000_0002,);

        assert!(registers.cpsr().carry(),);

        assert!(!result.branch);
    }

    #[test]
    fn lsr_zero_means_shift_by_thirty_two() {
        let mut registers = Registers::new();

        registers.write(0, 0x8000_0000);

        execute(
            &mut registers,
            ThumbInstruction::MoveShiftedRegister {
                operation: ThumbShiftOperation::LogicalRight,

                offset: 0,
                source: 0,
                destination: 1,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(1), 0,);

        assert!(registers.cpsr().carry(),);

        assert!(registers.cpsr().zero(),);
    }

    #[test]
    fn asr_zero_sign_extends() {
        let mut registers = Registers::new();

        registers.write(0, 0x8000_0000);

        execute(
            &mut registers,
            ThumbInstruction::MoveShiftedRegister {
                operation: ThumbShiftOperation::ArithmeticRight,

                offset: 0,
                source: 0,
                destination: 1,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(1), u32::MAX,);

        assert!(registers.cpsr().carry(),);

        assert!(registers.cpsr().negative(),);
    }

    #[test]
    fn add_register_updates_flags() {
        let mut registers = Registers::new();

        registers.write(1, u32::MAX);

        registers.write(2, 1);

        execute(
            &mut registers,
            ThumbInstruction::AddSubtract {
                operation: ThumbAddSubtractOperation::Add,

                operand: ThumbAddSubtractOperand::Register(2),

                source: 1,
                destination: 0,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(0), 0,);

        assert!(registers.cpsr().zero(),);

        assert!(registers.cpsr().carry(),);
    }

    #[test]
    fn subtract_sets_no_borrow_carry() {
        let mut registers = Registers::new();

        registers.write(1, 10);

        execute(
            &mut registers,
            ThumbInstruction::AddSubtract {
                operation: ThumbAddSubtractOperation::Subtract,

                operand: ThumbAddSubtractOperand::Immediate(3),

                source: 1,
                destination: 0,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(0), 7,);

        assert!(registers.cpsr().carry(),);
    }

    #[test]
    fn mov_immediate_preserves_carry_and_overflow() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_carry(true);

        registers.cpsr_mut().set_overflow(true);

        execute(
            &mut registers,
            ThumbInstruction::Immediate {
                operation: ThumbImmediateOperation::Move,

                destination: 0,
                immediate: 0,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(0), 0,);

        assert!(registers.cpsr().zero(),);

        assert!(registers.cpsr().carry(),);

        assert!(registers.cpsr().overflow(),);
    }

    #[test]
    fn compare_does_not_write_destination() {
        let mut registers = Registers::new();

        registers.write(0, 10);

        execute(
            &mut registers,
            ThumbInstruction::Immediate {
                operation: ThumbImmediateOperation::Compare,

                destination: 0,
                immediate: 10,
            },
            0x0800_0000,
        );

        assert_eq!(registers.read(0), 10,);

        assert!(registers.cpsr().zero(),);
    }

    #[test]
    fn bx_switches_to_arm_state() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_thumb_state(true);

        registers.write(0, 0x0800_0100);

        let result = execute(
            &mut registers,
            ThumbInstruction::HighRegister {
                operation: ThumbHighRegisterOperation::BranchExchange,

                source: 0,
                destination: 0,
            },
            0x0800_0000,
        );

        assert!(result.branch);

        assert_eq!(registers.pc(), 0x0800_0100,);

        assert!(!registers.cpsr().thumb_state(),);
    }

    #[test]
    fn bx_keeps_thumb_when_bit_zero_is_set() {
        let mut registers = Registers::new();

        registers.write(0, 0x0800_0101);

        execute(
            &mut registers,
            ThumbInstruction::HighRegister {
                operation: ThumbHighRegisterOperation::BranchExchange,

                source: 0,
                destination: 0,
            },
            0x0800_0000,
        );

        assert_eq!(registers.pc(), 0x0800_0100,);

        assert!(registers.cpsr().thumb_state(),);
    }

    #[test]
    fn conditional_branch_is_taken() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_zero(true);

        let result = execute(
            &mut registers,
            ThumbInstruction::ConditionalBranch {
                condition: ThumbCondition::Equal,

                offset: 4,
            },
            0x0800_0000,
        );

        assert!(result.branch);

        assert_eq!(registers.pc(), 0x0800_0008,);
    }

    #[test]
    fn conditional_branch_can_fail() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_zero(false);

        registers.set_pc(0x0800_0002);

        let result = execute(
            &mut registers,
            ThumbInstruction::ConditionalBranch {
                condition: ThumbCondition::Equal,

                offset: 4,
            },
            0x0800_0000,
        );

        assert!(!result.branch);

        assert_eq!(registers.pc(), 0x0800_0002,);
    }

    #[test]
    fn unconditional_branch_uses_pc_plus_four() {
        let mut registers = Registers::new();

        execute(
            &mut registers,
            ThumbInstruction::UnconditionalBranch { offset: 4 },
            0x0800_0000,
        );

        assert_eq!(registers.pc(), 0x0800_0008,);
    }

    #[test]
    fn thumb_swi_enters_supervisor_mode() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_mode(CpuMode::System);

        registers.cpsr_mut().set_thumb_state(true);

        let result = execute(
            &mut registers,
            ThumbInstruction::SoftwareInterrupt { comment: 0 },
            0x0800_0000,
        );

        assert!(result.branch);

        assert_eq!(registers.mode(), CpuMode::Supervisor,);

        assert_eq!(registers.pc(), 0x0000_0008,);

        assert_eq!(registers.read(Registers::LR,), 0x0800_0002,);

        assert!(!registers.cpsr().thumb_state(),);
    }
}
