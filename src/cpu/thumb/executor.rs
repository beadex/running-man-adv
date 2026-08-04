use crate::{
    bus::{AccessKind, Bus},
    cpu::{Exception, ExceptionError, Registers, enter_exception},
};

use super::{
    ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbAluOperation, ThumbCondition,
    ThumbHalfwordTransferKind, ThumbHighRegisterOperation, ThumbImmediateOperation,
    ThumbImmediateTransferKind, ThumbInstruction, ThumbLoadAddressBase, ThumbLongBranchHalf,
    ThumbRegisterOffsetTransferKind, ThumbShiftOperation, ThumbSpRelativeTransferKind,
    ThumbStackPointerOperation,
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
    bus: &mut Bus,
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

        ThumbInstruction::Alu {
            operation,
            source,
            destination,
        } => {
            let cycles = execute_alu(registers, *operation, *source, *destination);
            Ok(ThumbExecutionResult::sequential(cycles))
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

        ThumbInstruction::LiteralLoad {
            destination,
            offset,
        } => {
            let cycles =
                execute_literal_load(registers, bus, *destination, *offset, instruction_address);
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::RegisterOffsetTransfer {
            kind,
            offset_register,
            base_register,
            destination,
        } => {
            let cycles = execute_register_offset_transfer(
                registers,
                bus,
                *kind,
                *offset_register,
                *base_register,
                *destination,
            );
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::ImmediateOffsetTransfer {
            kind,
            offset,
            base_register,
            destination,
        } => {
            let cycles = execute_immediate_offset_transfer(
                registers,
                bus,
                *kind,
                *offset,
                *base_register,
                *destination,
            );
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::HalfwordImmediateTransfer {
            kind,
            offset,
            base_register,
            destination,
        } => {
            let cycles = execute_halfword_immediate_transfer(
                registers,
                bus,
                *kind,
                *offset,
                *base_register,
                *destination,
            );
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::SpRelativeTransfer {
            kind,
            destination,
            offset,
        } => {
            let cycles = execute_sp_relative_transfer(registers, bus, *kind, *destination, *offset);
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::LoadAddress {
            base,
            destination,
            offset,
        } => {
            execute_load_address(registers, *base, *destination, *offset, instruction_address);
            Ok(ThumbExecutionResult::sequential(1))
        }

        ThumbInstruction::AdjustStackPointer { operation, offset } => {
            execute_adjust_stack_pointer(registers, *operation, *offset);
            Ok(ThumbExecutionResult::sequential(1))
        }

        ThumbInstruction::Push {
            registers: register_list,
            include_link_register,
        } => {
            let cycles = execute_push(registers, bus, *register_list, *include_link_register);
            Ok(ThumbExecutionResult::sequential(cycles))
        }

        ThumbInstruction::Pop {
            registers: register_list,
            include_program_counter,
        } => {
            let (cycles, branch) =
                execute_pop(registers, bus, *register_list, *include_program_counter);
            Ok(ThumbExecutionResult { cycles, branch })
        }

        ThumbInstruction::MultipleTransfer {
            load,
            base_register,
            registers: register_list,
        } => {
            let cycles =
                execute_multiple_transfer(registers, bus, *load, *base_register, *register_list);
            Ok(ThumbExecutionResult::sequential(cycles))
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

        ThumbInstruction::LongBranchWithLink { half, offset } => {
            let branch =
                execute_long_branch_with_link(registers, *half, *offset, instruction_address);

            Ok(ThumbExecutionResult { cycles: 1, branch })
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

fn execute_alu(
    registers: &mut Registers,
    operation: ThumbAluOperation,
    source: u8,
    destination: u8,
) -> u32 {
    let left = registers.read(destination as usize);
    let right = registers.read(source as usize);
    let old_carry = registers.cpsr().carry();
    let old_overflow = registers.cpsr().overflow();

    match operation {
        ThumbAluOperation::And => {
            let value = left & right;
            registers.write(destination as usize, value);
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
        }

        ThumbAluOperation::ExclusiveOr => {
            let value = left ^ right;
            registers.write(destination as usize, value);
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
        }

        ThumbAluOperation::LogicalShiftLeft => {
            let result = logical_shift_left_register(left, (right & 0xFF) as u8, old_carry);
            registers.write(destination as usize, result.value);
            apply_logical_flags(registers, result.value, result.carry, old_overflow);
            1
        }

        ThumbAluOperation::LogicalShiftRight => {
            let result = logical_shift_right_register(left, (right & 0xFF) as u8, old_carry);
            registers.write(destination as usize, result.value);
            apply_logical_flags(registers, result.value, result.carry, old_overflow);
            1
        }

        ThumbAluOperation::ArithmeticShiftRight => {
            let result = arithmetic_shift_right_register(left, (right & 0xFF) as u8, old_carry);
            registers.write(destination as usize, result.value);
            apply_logical_flags(registers, result.value, result.carry, old_overflow);
            1
        }

        ThumbAluOperation::AddWithCarry => {
            let result = add_with_carry(left, right, old_carry);
            registers.write(destination as usize, result.value);
            apply_arithmetic_flags(registers, result);
            1
        }

        ThumbAluOperation::SubtractWithCarry => {
            let result = subtract_with_carry(left, right, old_carry);
            registers.write(destination as usize, result.value);
            apply_arithmetic_flags(registers, result);
            1
        }

        ThumbAluOperation::RotateRight => {
            let result = rotate_right_register(left, (right & 0xFF) as u8, old_carry);
            registers.write(destination as usize, result.value);
            apply_logical_flags(registers, result.value, result.carry, old_overflow);
            1
        }

        ThumbAluOperation::Test => {
            let value = left & right;
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
        }

        ThumbAluOperation::Negate => {
            let result = subtract_with_flags(0, right);
            registers.write(destination as usize, result.value);
            apply_arithmetic_flags(registers, result);
            1
        }

        ThumbAluOperation::Compare => {
            let result = subtract_with_flags(left, right);
            apply_arithmetic_flags(registers, result);
            1
        }

        ThumbAluOperation::CompareNegative => {
            let result = add_with_flags(left, right);
            apply_arithmetic_flags(registers, result);
            1
        }

        ThumbAluOperation::Or => {
            let value = left | right;
            registers.write(destination as usize, value);
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
        }

        ThumbAluOperation::Multiply => {
            let value = left.wrapping_mul(right);
            registers.write(destination as usize, value);

            /*
             * ARM7TDMI MUL updates N and Z while C and V are architecturally
             * meaningless/preserved by this interpreter.
             */
            apply_logical_flags(registers, value, old_carry, old_overflow);

            multiply_internal_cycles(right)
        }

        ThumbAluOperation::BitClear => {
            let value = left & !right;
            registers.write(destination as usize, value);
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
        }

        ThumbAluOperation::MoveNot => {
            let value = !right;
            registers.write(destination as usize, value);
            apply_logical_flags(registers, value, old_carry, old_overflow);
            1
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

fn execute_literal_load(
    registers: &mut Registers,
    bus: &Bus,
    destination: u8,
    offset: u16,
    instruction_address: u32,
) -> u32 {
    let address = (instruction_address.wrapping_add(4) & !3).wrapping_add(offset as u32);
    let access = bus.read32_timed(address, AccessKind::NonSequential);
    registers.write(destination as usize, access.value);
    access.cycles.saturating_add(1)
}

fn execute_register_offset_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    kind: ThumbRegisterOffsetTransferKind,
    offset_register: u8,
    base_register: u8,
    destination: u8,
) -> u32 {
    let address = registers
        .read(base_register as usize)
        .wrapping_add(registers.read(offset_register as usize));

    match kind {
        ThumbRegisterOffsetTransferKind::StoreWord => bus.write32_timed(
            address & !3,
            registers.read(destination as usize),
            AccessKind::NonSequential,
        ),
        ThumbRegisterOffsetTransferKind::StoreHalfword => bus.write16_timed(
            address & !1,
            registers.read(destination as usize) as u16,
            AccessKind::NonSequential,
        ),
        ThumbRegisterOffsetTransferKind::StoreByte => bus.write8_timed(
            address,
            registers.read(destination as usize) as u8,
            AccessKind::NonSequential,
        ),
        ThumbRegisterOffsetTransferKind::LoadSignedByte => {
            let access = bus.read8_timed(address, AccessKind::NonSequential);
            registers.write(destination as usize, (access.value as i8) as i32 as u32);
            access.cycles.saturating_add(1)
        }
        ThumbRegisterOffsetTransferKind::LoadWord => {
            let access = bus.read32_timed(address & !3, AccessKind::NonSequential);
            let value = access.value.rotate_right((address & 3) * 8);
            registers.write(destination as usize, value);
            access.cycles.saturating_add(1)
        }
        ThumbRegisterOffsetTransferKind::LoadHalfword => {
            let access = bus.read16_timed(address & !1, AccessKind::NonSequential);
            let value = if address & 1 == 0 {
                access.value as u32
            } else {
                (access.value as u32).rotate_right(8)
            };
            registers.write(destination as usize, value);
            access.cycles.saturating_add(1)
        }
        ThumbRegisterOffsetTransferKind::LoadByte => {
            let access = bus.read8_timed(address, AccessKind::NonSequential);
            registers.write(destination as usize, access.value as u32);
            access.cycles.saturating_add(1)
        }
        ThumbRegisterOffsetTransferKind::LoadSignedHalfword => {
            if address & 1 != 0 {
                let access = bus.read8_timed(address, AccessKind::NonSequential);
                registers.write(destination as usize, (access.value as i8) as i32 as u32);
                access.cycles.saturating_add(1)
            } else {
                let access = bus.read16_timed(address, AccessKind::NonSequential);
                registers.write(destination as usize, (access.value as i16) as i32 as u32);
                access.cycles.saturating_add(1)
            }
        }
    }
}

fn execute_immediate_offset_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    kind: ThumbImmediateTransferKind,
    offset: u8,
    base_register: u8,
    destination: u8,
) -> u32 {
    let address = registers
        .read(base_register as usize)
        .wrapping_add(offset as u32);

    match kind {
        ThumbImmediateTransferKind::StoreWord => bus.write32_timed(
            address & !3,
            registers.read(destination as usize),
            AccessKind::NonSequential,
        ),
        ThumbImmediateTransferKind::LoadWord => {
            let access = bus.read32_timed(address & !3, AccessKind::NonSequential);
            registers.write(
                destination as usize,
                access.value.rotate_right((address & 3) * 8),
            );
            access.cycles.saturating_add(1)
        }
        ThumbImmediateTransferKind::StoreByte => bus.write8_timed(
            address,
            registers.read(destination as usize) as u8,
            AccessKind::NonSequential,
        ),
        ThumbImmediateTransferKind::LoadByte => {
            let access = bus.read8_timed(address, AccessKind::NonSequential);
            registers.write(destination as usize, access.value as u32);
            access.cycles.saturating_add(1)
        }
    }
}

fn execute_halfword_immediate_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    kind: ThumbHalfwordTransferKind,
    offset: u8,
    base_register: u8,
    destination: u8,
) -> u32 {
    let address = registers
        .read(base_register as usize)
        .wrapping_add(offset as u32);

    match kind {
        ThumbHalfwordTransferKind::Store => bus.write16_timed(
            address & !1,
            registers.read(destination as usize) as u16,
            AccessKind::NonSequential,
        ),
        ThumbHalfwordTransferKind::Load => {
            let access = bus.read16_timed(address & !1, AccessKind::NonSequential);
            let value = if address & 1 == 0 {
                access.value as u32
            } else {
                (access.value as u32).rotate_right(8)
            };
            registers.write(destination as usize, value);
            access.cycles.saturating_add(1)
        }
    }
}

fn execute_sp_relative_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    kind: ThumbSpRelativeTransferKind,
    destination: u8,
    offset: u16,
) -> u32 {
    let address = registers.read(Registers::SP).wrapping_add(offset as u32);

    match kind {
        ThumbSpRelativeTransferKind::Store => bus.write32_timed(
            address & !3,
            registers.read(destination as usize),
            AccessKind::NonSequential,
        ),
        ThumbSpRelativeTransferKind::Load => {
            let access = bus.read32_timed(address & !3, AccessKind::NonSequential);
            registers.write(
                destination as usize,
                access.value.rotate_right((address & 3) * 8),
            );
            access.cycles.saturating_add(1)
        }
    }
}

fn execute_load_address(
    registers: &mut Registers,
    base: ThumbLoadAddressBase,
    destination: u8,
    offset: u16,
    instruction_address: u32,
) {
    let base = match base {
        ThumbLoadAddressBase::ProgramCounter => instruction_address.wrapping_add(4) & !3,
        ThumbLoadAddressBase::StackPointer => registers.read(Registers::SP),
    };
    registers.write(destination as usize, base.wrapping_add(offset as u32));
}

fn execute_adjust_stack_pointer(
    registers: &mut Registers,
    operation: ThumbStackPointerOperation,
    offset: u16,
) {
    let stack_pointer = registers.read(Registers::SP);
    let value = match operation {
        ThumbStackPointerOperation::Add => stack_pointer.wrapping_add(offset as u32),
        ThumbStackPointerOperation::Subtract => stack_pointer.wrapping_sub(offset as u32),
    };
    registers.write(Registers::SP, value);
}

fn execute_push(
    registers: &mut Registers,
    bus: &mut Bus,
    register_list: u8,
    include_link_register: bool,
) -> u32 {
    let count = register_list.count_ones() + u32::from(include_link_register);
    if count == 0 {
        return 1;
    }

    let original_sp = registers.read(Registers::SP);
    let new_sp = original_sp.wrapping_sub(count * 4);
    let mut address = new_sp;
    let mut cycles: u32 = 0;
    let mut first = true;

    for register in 0..8u8 {
        if register_list & (1 << register) == 0 {
            continue;
        }
        cycles = cycles.saturating_add(bus.write32_timed(
            address,
            registers.read(register as usize),
            access_kind(&mut first),
        ));
        address = address.wrapping_add(4);
    }

    if include_link_register {
        cycles = cycles.saturating_add(bus.write32_timed(
            address,
            registers.read(Registers::LR),
            access_kind(&mut first),
        ));
    }

    registers.write(Registers::SP, new_sp);
    cycles
}

fn execute_pop(
    registers: &mut Registers,
    bus: &Bus,
    register_list: u8,
    include_program_counter: bool,
) -> (u32, bool) {
    let count = register_list.count_ones() + u32::from(include_program_counter);
    if count == 0 {
        return (1, false);
    }

    let original_sp = registers.read(Registers::SP);
    let mut address = original_sp;
    let mut cycles: u32 = 0;
    let mut first = true;

    for register in 0..8u8 {
        if register_list & (1 << register) == 0 {
            continue;
        }
        let access = bus.read32_timed(address, access_kind(&mut first));
        cycles = cycles.saturating_add(access.cycles);
        registers.write(register as usize, access.value);
        address = address.wrapping_add(4);
    }

    let branch = if include_program_counter {
        let access = bus.read32_timed(address, access_kind(&mut first));
        cycles = cycles.saturating_add(access.cycles);
        registers.set_pc(access.value & !1);
        true
    } else {
        false
    };

    registers.write(Registers::SP, original_sp.wrapping_add(count * 4));
    (cycles.saturating_add(1), branch)
}

fn execute_multiple_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    load: bool,
    base_register: u8,
    register_list: u8,
) -> u32 {
    let original_base = registers.read(base_register as usize);

    // ARM7TDMI gives the empty-list form special PC transfer semantics.
    // Keep it deterministic and non-destructive until that edge case is modelled.
    if register_list == 0 {
        return 1;
    }

    let mut address = original_base;
    let mut cycles: u32 = 0;
    let mut first = true;

    if load {
        for register in 0..8u8 {
            if register_list & (1 << register) == 0 {
                continue;
            }
            let access = bus.read32_timed(address, access_kind(&mut first));
            cycles = cycles.saturating_add(access.cycles);
            registers.write(register as usize, access.value);
            address = address.wrapping_add(4);
        }

        // When Rb is in an LDMIA list, suppress write-back so the loaded value wins.
        if register_list & (1 << base_register) == 0 {
            registers.write(base_register as usize, address);
        }
        cycles.saturating_add(1)
    } else {
        for register in 0..8u8 {
            if register_list & (1 << register) == 0 {
                continue;
            }
            let value = if register == base_register {
                original_base
            } else {
                registers.read(register as usize)
            };
            cycles =
                cycles.saturating_add(bus.write32_timed(address, value, access_kind(&mut first)));
            address = address.wrapping_add(4);
        }
        registers.write(base_register as usize, address);
        cycles
    }
}

fn access_kind(first: &mut bool) -> AccessKind {
    if *first {
        *first = false;
        AccessKind::NonSequential
    } else {
        AccessKind::Sequential
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

fn execute_long_branch_with_link(
    registers: &mut Registers,
    half: ThumbLongBranchHalf,
    offset: i32,
    instruction_address: u32,
) -> bool {
    match half {
        ThumbLongBranchHalf::First => {
            /*
             * First half:
             *
             * LR = current instruction address + 4 + sign_extend(imm11 << 12)
             */
            let base = instruction_address.wrapping_add(4);
            registers.write(Registers::LR, base.wrapping_add(offset as u32));

            false
        }

        ThumbLongBranchHalf::Second => {
            /*
             * Second half uses the partial target stored in LR by the first
             * half. The return address is the following THUMB instruction,
             * with bit zero set so BX LR returns in THUMB state.
             */
            let target = registers.read(Registers::LR).wrapping_add(offset as u32);

            let return_address = instruction_address.wrapping_add(2) | 1;

            registers.write(Registers::LR, return_address);
            registers.set_pc(target & !1);

            true
        }
    }
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

fn logical_shift_left_register(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    match amount {
        0 => ShiftResult {
            value,
            carry: old_carry,
        },

        1..=31 => ShiftResult {
            value: value << amount,
            carry: value & (1u32 << (32 - amount as u32)) != 0,
        },

        32 => ShiftResult {
            value: 0,
            carry: value & 1 != 0,
        },

        _ => ShiftResult {
            value: 0,
            carry: false,
        },
    }
}

fn logical_shift_right_register(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    match amount {
        0 => ShiftResult {
            value,
            carry: old_carry,
        },

        1..=31 => ShiftResult {
            value: value >> amount,
            carry: value & (1u32 << (amount as u32 - 1)) != 0,
        },

        32 => ShiftResult {
            value: 0,
            carry: value & 0x8000_0000 != 0,
        },

        _ => ShiftResult {
            value: 0,
            carry: false,
        },
    }
}

fn arithmetic_shift_right_register(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    match amount {
        0 => ShiftResult {
            value,
            carry: old_carry,
        },

        1..=31 => ShiftResult {
            value: ((value as i32) >> amount) as u32,
            carry: value & (1u32 << (amount as u32 - 1)) != 0,
        },

        _ => {
            let sign = value & 0x8000_0000 != 0;
            ShiftResult {
                value: if sign { u32::MAX } else { 0 },
                carry: sign,
            }
        }
    }
}

fn rotate_right_register(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    if amount == 0 {
        return ShiftResult {
            value,
            carry: old_carry,
        };
    }

    let rotation = (amount as u32) & 31;

    if rotation == 0 {
        ShiftResult {
            value,
            carry: value & 0x8000_0000 != 0,
        }
    } else {
        let value = value.rotate_right(rotation);
        ShiftResult {
            value,
            carry: value & 0x8000_0000 != 0,
        }
    }
}

fn add_with_carry(left: u32, right: u32, carry_in: bool) -> ArithmeticResult {
    let unsigned_sum = left as u64 + right as u64 + carry_in as u64;
    let value = unsigned_sum as u32;
    let carry = unsigned_sum > u32::MAX as u64;

    let signed_sum = left as i32 as i64 + right as i32 as i64 + carry_in as i64;
    let overflow = signed_sum < i32::MIN as i64 || signed_sum > i32::MAX as i64;

    ArithmeticResult {
        value,
        carry,
        overflow,
    }
}

fn subtract_with_carry(left: u32, right: u32, carry_in: bool) -> ArithmeticResult {
    /*
     * SBC computes left - right - (1 - C).
     */
    let borrow_in = (!carry_in) as u64;
    let subtrahend = right as u64 + borrow_in;
    let value = left.wrapping_sub(right).wrapping_sub(borrow_in as u32);
    let carry = left as u64 >= subtrahend;

    let signed_result = left as i32 as i64 - right as i32 as i64 - borrow_in as i64;
    let overflow = signed_result < i32::MIN as i64 || signed_result > i32::MAX as i64;

    ArithmeticResult {
        value,
        carry,
        overflow,
    }
}

fn apply_logical_flags(registers: &mut Registers, value: u32, carry: bool, overflow: bool) {
    registers
        .cpsr_mut()
        .set_nzcv(value & 0x8000_0000 != 0, value == 0, carry, overflow);
}

fn multiply_internal_cycles(multiplier: u32) -> u32 {
    /*
     * Preliminary ARM7TDMI-style early termination model.
     *
     * The multiplier can finish earlier when its upper bytes are all zero
     * or all one. Return at least one execution cycle.
     */
    if multiplier & 0xFFFF_FF00 == 0 || multiplier & 0xFFFF_FF00 == 0xFFFF_FF00 {
        1
    } else if multiplier & 0xFFFF_0000 == 0 || multiplier & 0xFFFF_0000 == 0xFFFF_0000 {
        2
    } else if multiplier & 0xFF00_0000 == 0 || multiplier & 0xFF00_0000 == 0xFF00_0000 {
        3
    } else {
        4
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

    #[test]
    fn literal_load_uses_aligned_pc_plus_four() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();
        bus.write32(0x0200_0108, 0x1234_5678);

        execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::LiteralLoad {
                destination: 0,
                offset: 4,
            },
            0x0200_0102,
        )
        .unwrap();

        assert_eq!(registers.read(0), 0x1234_5678);
    }

    #[test]
    fn word_load_rotates_unaligned_value() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();
        registers.write(1, 0x0200_0101);
        bus.write32(0x0200_0100, 0x1122_3344);

        execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::ImmediateOffsetTransfer {
                kind: crate::cpu::thumb::ThumbImmediateTransferKind::LoadWord,
                offset: 0,
                base_register: 1,
                destination: 0,
            },
            0,
        )
        .unwrap();

        assert_eq!(registers.read(0), 0x4411_2233);
    }

    #[test]
    fn push_and_pop_round_trip_registers_and_pc() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();
        registers.write(Registers::SP, 0x0300_7F00);
        registers.write(0, 0xAAAA_AAAA);
        registers.write(1, 0xBBBB_BBBB);
        registers.write(Registers::LR, 0x0800_0101);

        execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::Push {
                registers: 0b0000_0011,
                include_link_register: true,
            },
            0,
        )
        .unwrap();

        registers.write(0, 0);
        registers.write(1, 0);

        let result = execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::Pop {
                registers: 0b0000_0011,
                include_program_counter: true,
            },
            0,
        )
        .unwrap();

        assert!(result.branch);
        assert_eq!(registers.read(0), 0xAAAA_AAAA);
        assert_eq!(registers.read(1), 0xBBBB_BBBB);
        assert_eq!(registers.pc(), 0x0800_0100);
        assert_eq!(registers.read(Registers::SP), 0x0300_7F00);
    }

    #[test]
    fn stmia_and_ldmia_transfer_register_list() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();
        registers.write(0, 0x0200_0100);
        registers.write(1, 0x1111_1111);
        registers.write(2, 0x2222_2222);

        execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::MultipleTransfer {
                load: false,
                base_register: 0,
                registers: 0b0000_0110,
            },
            0,
        )
        .unwrap();

        assert_eq!(bus.read32(0x0200_0100), 0x1111_1111);
        assert_eq!(bus.read32(0x0200_0104), 0x2222_2222);
        assert_eq!(registers.read(0), 0x0200_0108);

        registers.write(3, 0x0200_0100);
        execute_thumb(
            &mut registers,
            &mut bus,
            &ThumbInstruction::MultipleTransfer {
                load: true,
                base_register: 3,
                registers: 0b0011_0000,
            },
            0,
        )
        .unwrap();

        assert_eq!(registers.read(4), 0x1111_1111);
        assert_eq!(registers.read(5), 0x2222_2222);
        assert_eq!(registers.read(3), 0x0200_0108);
    }

    #[test]
    fn alu_adc_uses_carry_input() {
        let mut registers = Registers::new();
        registers.write(0, u32::MAX);
        registers.write(1, 0);
        registers.cpsr_mut().set_carry(true);

        execute(
            &mut registers,
            ThumbInstruction::Alu {
                operation: crate::cpu::thumb::ThumbAluOperation::AddWithCarry,
                source: 1,
                destination: 0,
            },
            0,
        );

        assert_eq!(registers.read(0), 0);
        assert!(registers.cpsr().zero());
        assert!(registers.cpsr().carry());
    }

    #[test]
    fn alu_register_shift_zero_preserves_carry() {
        let mut registers = Registers::new();
        registers.write(0, 0x8000_0001);
        registers.write(1, 0);
        registers.cpsr_mut().set_carry(true);

        execute(
            &mut registers,
            ThumbInstruction::Alu {
                operation: crate::cpu::thumb::ThumbAluOperation::LogicalShiftRight,
                source: 1,
                destination: 0,
            },
            0,
        );

        assert_eq!(registers.read(0), 0x8000_0001);
        assert!(registers.cpsr().carry());
    }

    #[test]
    fn alu_negate_sets_subtraction_flags() {
        let mut registers = Registers::new();
        registers.write(1, 1);

        execute(
            &mut registers,
            ThumbInstruction::Alu {
                operation: crate::cpu::thumb::ThumbAluOperation::Negate,
                source: 1,
                destination: 0,
            },
            0,
        );

        assert_eq!(registers.read(0), u32::MAX);
        assert!(registers.cpsr().negative());
        assert!(!registers.cpsr().carry());
    }

    #[test]
    fn alu_multiply_writes_result_and_nz() {
        let mut registers = Registers::new();
        registers.write(0, 7);
        registers.write(1, 6);

        let result = execute(
            &mut registers,
            ThumbInstruction::Alu {
                operation: crate::cpu::thumb::ThumbAluOperation::Multiply,
                source: 1,
                destination: 0,
            },
            0,
        );

        assert_eq!(registers.read(0), 42);
        assert!(!registers.cpsr().zero());
        assert!(result.cycles >= 1);
    }

    #[test]
    fn long_branch_with_link_builds_target_and_return_address() {
        let mut registers = Registers::new();
        registers.cpsr_mut().set_thumb_state(true);

        let first = execute(
            &mut registers,
            ThumbInstruction::LongBranchWithLink {
                half: crate::cpu::thumb::ThumbLongBranchHalf::First,
                offset: 0x1000,
            },
            0x0800_0000,
        );

        assert!(!first.branch);
        assert_eq!(registers.read(Registers::LR), 0x0800_1004);

        let second = execute(
            &mut registers,
            ThumbInstruction::LongBranchWithLink {
                half: crate::cpu::thumb::ThumbLongBranchHalf::Second,
                offset: 6,
            },
            0x0800_0002,
        );

        assert!(second.branch);
        assert_eq!(registers.pc(), 0x0800_100A);
        assert_eq!(registers.read(Registers::LR), 0x0800_0005);
        assert!(registers.cpsr().thumb_state());
    }
}
