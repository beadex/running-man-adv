use super::{
    ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbAluOperation, ThumbCondition,
    ThumbHalfwordTransferKind, ThumbHighRegisterOperation, ThumbImmediateOperation,
    ThumbImmediateTransferKind, ThumbInstruction, ThumbLoadAddressBase, ThumbLongBranchHalf,
    ThumbRegisterOffsetTransferKind, ThumbShiftOperation, ThumbSpRelativeTransferKind,
    ThumbStackPointerOperation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbDecodeError {
    UndefinedConditionalBranch,
    UnsupportedInstruction { raw: u16 },
}

pub fn decode_thumb(raw: u16) -> Result<ThumbInstruction, ThumbDecodeError> {
    if raw & 0xE000 == 0x0000 && ((raw >> 11) & 0b11) != 0b11 {
        return Ok(decode_move_shifted_register(raw));
    }

    if raw & 0xF800 == 0x1800 {
        return Ok(decode_add_subtract(raw));
    }

    if raw & 0xE000 == 0x2000 {
        return Ok(decode_immediate(raw));
    }

    if raw & 0xFC00 == 0x4000 {
        return Ok(decode_alu(raw));
    }

    if raw & 0xFC00 == 0x4400 {
        return Ok(decode_high_register(raw));
    }

    if raw & 0xF800 == 0x4800 {
        return Ok(ThumbInstruction::LiteralLoad {
            destination: ((raw >> 8) & 7) as u8,
            offset: (raw & 0xFF) << 2,
        });
    }

    if raw & 0xF000 == 0x5000 {
        return Ok(decode_register_offset_transfer(raw));
    }

    if raw & 0xE000 == 0x6000 {
        return Ok(decode_immediate_offset_transfer(raw));
    }

    if raw & 0xF000 == 0x8000 {
        return Ok(ThumbInstruction::HalfwordImmediateTransfer {
            kind: if raw & (1 << 11) != 0 {
                ThumbHalfwordTransferKind::Load
            } else {
                ThumbHalfwordTransferKind::Store
            },
            offset: (((raw >> 6) & 0x1F) as u8) << 1,
            base_register: ((raw >> 3) & 7) as u8,
            destination: (raw & 7) as u8,
        });
    }

    if raw & 0xF000 == 0x9000 {
        return Ok(ThumbInstruction::SpRelativeTransfer {
            kind: if raw & (1 << 11) != 0 {
                ThumbSpRelativeTransferKind::Load
            } else {
                ThumbSpRelativeTransferKind::Store
            },
            destination: ((raw >> 8) & 7) as u8,
            offset: (raw & 0xFF) << 2,
        });
    }

    if raw & 0xF000 == 0xA000 {
        return Ok(ThumbInstruction::LoadAddress {
            base: if raw & (1 << 11) != 0 {
                ThumbLoadAddressBase::StackPointer
            } else {
                ThumbLoadAddressBase::ProgramCounter
            },
            destination: ((raw >> 8) & 7) as u8,
            offset: (raw & 0xFF) << 2,
        });
    }

    if raw & 0xFF00 == 0xB000 {
        return Ok(ThumbInstruction::AdjustStackPointer {
            operation: if raw & (1 << 7) != 0 {
                ThumbStackPointerOperation::Subtract
            } else {
                ThumbStackPointerOperation::Add
            },
            offset: (raw & 0x7F) << 2,
        });
    }

    if raw & 0xFE00 == 0xB400 {
        return Ok(ThumbInstruction::Push {
            registers: (raw & 0xFF) as u8,
            include_link_register: raw & (1 << 8) != 0,
        });
    }

    if raw & 0xFE00 == 0xBC00 {
        return Ok(ThumbInstruction::Pop {
            registers: (raw & 0xFF) as u8,
            include_program_counter: raw & (1 << 8) != 0,
        });
    }

    if raw & 0xF000 == 0xC000 {
        return Ok(ThumbInstruction::MultipleTransfer {
            load: raw & (1 << 11) != 0,
            base_register: ((raw >> 8) & 7) as u8,
            registers: (raw & 0xFF) as u8,
        });
    }

    if raw & 0xF000 == 0xD000 {
        return decode_conditional_or_swi(raw);
    }

    if raw & 0xF800 == 0xE000 {
        return Ok(decode_unconditional_branch(raw));
    }

    if raw & 0xF800 == 0xF000 {
        return Ok(ThumbInstruction::LongBranchWithLink {
            half: ThumbLongBranchHalf::First,
            offset: sign_extend_11(raw & 0x07FF) << 12,
        });
    }

    if raw & 0xF800 == 0xF800 {
        return Ok(ThumbInstruction::LongBranchWithLink {
            half: ThumbLongBranchHalf::Second,
            offset: ((raw & 0x07FF) as i32) << 1,
        });
    }

    Err(ThumbDecodeError::UnsupportedInstruction { raw })
}

fn decode_move_shifted_register(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 11) & 3 {
        0 => ThumbShiftOperation::LogicalLeft,
        1 => ThumbShiftOperation::LogicalRight,
        2 => ThumbShiftOperation::ArithmeticRight,
        _ => unreachable!(),
    };

    ThumbInstruction::MoveShiftedRegister {
        operation,
        offset: ((raw >> 6) & 0x1F) as u8,
        source: ((raw >> 3) & 7) as u8,
        destination: (raw & 7) as u8,
    }
}

fn decode_add_subtract(raw: u16) -> ThumbInstruction {
    let value = ((raw >> 6) & 7) as u8;
    ThumbInstruction::AddSubtract {
        operation: if raw & (1 << 9) != 0 {
            ThumbAddSubtractOperation::Subtract
        } else {
            ThumbAddSubtractOperation::Add
        },
        operand: if raw & (1 << 10) != 0 {
            ThumbAddSubtractOperand::Immediate(value)
        } else {
            ThumbAddSubtractOperand::Register(value)
        },
        source: ((raw >> 3) & 7) as u8,
        destination: (raw & 7) as u8,
    }
}

fn decode_immediate(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 11) & 3 {
        0 => ThumbImmediateOperation::Move,
        1 => ThumbImmediateOperation::Compare,
        2 => ThumbImmediateOperation::Add,
        3 => ThumbImmediateOperation::Subtract,
        _ => unreachable!(),
    };

    ThumbInstruction::Immediate {
        operation,
        destination: ((raw >> 8) & 7) as u8,
        immediate: (raw & 0xFF) as u8,
    }
}

fn decode_alu(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 6) & 0x0F {
        0x0 => ThumbAluOperation::And,
        0x1 => ThumbAluOperation::ExclusiveOr,
        0x2 => ThumbAluOperation::LogicalShiftLeft,
        0x3 => ThumbAluOperation::LogicalShiftRight,
        0x4 => ThumbAluOperation::ArithmeticShiftRight,
        0x5 => ThumbAluOperation::AddWithCarry,
        0x6 => ThumbAluOperation::SubtractWithCarry,
        0x7 => ThumbAluOperation::RotateRight,
        0x8 => ThumbAluOperation::Test,
        0x9 => ThumbAluOperation::Negate,
        0xA => ThumbAluOperation::Compare,
        0xB => ThumbAluOperation::CompareNegative,
        0xC => ThumbAluOperation::Or,
        0xD => ThumbAluOperation::Multiply,
        0xE => ThumbAluOperation::BitClear,
        0xF => ThumbAluOperation::MoveNot,
        _ => unreachable!(),
    };

    ThumbInstruction::Alu {
        operation,
        source: ((raw >> 3) & 7) as u8,
        destination: (raw & 7) as u8,
    }
}

fn decode_high_register(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 8) & 3 {
        0 => ThumbHighRegisterOperation::Add,
        1 => ThumbHighRegisterOperation::Compare,
        2 => ThumbHighRegisterOperation::Move,
        3 => ThumbHighRegisterOperation::BranchExchange,
        _ => unreachable!(),
    };

    ThumbInstruction::HighRegister {
        operation,
        source: (((raw >> 3) & 7) | (((raw >> 6) & 1) << 3)) as u8,
        destination: ((raw & 7) | (((raw >> 7) & 1) << 3)) as u8,
    }
}

fn decode_register_offset_transfer(raw: u16) -> ThumbInstruction {
    let kind = match (raw >> 9) & 7 {
        0 => ThumbRegisterOffsetTransferKind::StoreWord,
        1 => ThumbRegisterOffsetTransferKind::StoreHalfword,
        2 => ThumbRegisterOffsetTransferKind::StoreByte,
        3 => ThumbRegisterOffsetTransferKind::LoadSignedByte,
        4 => ThumbRegisterOffsetTransferKind::LoadWord,
        5 => ThumbRegisterOffsetTransferKind::LoadHalfword,
        6 => ThumbRegisterOffsetTransferKind::LoadByte,
        7 => ThumbRegisterOffsetTransferKind::LoadSignedHalfword,
        _ => unreachable!(),
    };

    ThumbInstruction::RegisterOffsetTransfer {
        kind,
        offset_register: ((raw >> 6) & 7) as u8,
        base_register: ((raw >> 3) & 7) as u8,
        destination: (raw & 7) as u8,
    }
}

fn decode_immediate_offset_transfer(raw: u16) -> ThumbInstruction {
    let byte = raw & (1 << 12) != 0;
    let load = raw & (1 << 11) != 0;
    let encoded_offset = ((raw >> 6) & 0x1F) as u8;

    let kind = match (load, byte) {
        (false, false) => ThumbImmediateTransferKind::StoreWord,
        (true, false) => ThumbImmediateTransferKind::LoadWord,
        (false, true) => ThumbImmediateTransferKind::StoreByte,
        (true, true) => ThumbImmediateTransferKind::LoadByte,
    };

    ThumbInstruction::ImmediateOffsetTransfer {
        kind,
        offset: if byte {
            encoded_offset
        } else {
            encoded_offset << 2
        },
        base_register: ((raw >> 3) & 7) as u8,
        destination: (raw & 7) as u8,
    }
}

fn decode_conditional_or_swi(raw: u16) -> Result<ThumbInstruction, ThumbDecodeError> {
    let bits = ((raw >> 8) & 0xF) as u8;
    let immediate = (raw & 0xFF) as u8;
    if bits == 0xF {
        return Ok(ThumbInstruction::SoftwareInterrupt { comment: immediate });
    }
    if bits == 0xE {
        return Err(ThumbDecodeError::UndefinedConditionalBranch);
    }

    Ok(ThumbInstruction::ConditionalBranch {
        condition: decode_condition(bits),
        offset: ((immediate as i8) as i16) << 1,
    })
}

fn decode_condition(bits: u8) -> ThumbCondition {
    match bits {
        0x0 => ThumbCondition::Equal,
        0x1 => ThumbCondition::NotEqual,
        0x2 => ThumbCondition::CarrySet,
        0x3 => ThumbCondition::CarryClear,
        0x4 => ThumbCondition::Minus,
        0x5 => ThumbCondition::Plus,
        0x6 => ThumbCondition::Overflow,
        0x7 => ThumbCondition::NoOverflow,
        0x8 => ThumbCondition::UnsignedHigher,
        0x9 => ThumbCondition::UnsignedLowerOrSame,
        0xA => ThumbCondition::SignedGreaterOrEqual,
        0xB => ThumbCondition::SignedLessThan,
        0xC => ThumbCondition::SignedGreaterThan,
        0xD => ThumbCondition::SignedLessOrEqual,
        _ => unreachable!(),
    }
}

fn decode_unconditional_branch(raw: u16) -> ThumbInstruction {
    ThumbInstruction::UnconditionalBranch {
        offset: sign_extend_11(raw & 0x07FF) << 1,
    }
}

fn sign_extend_11(value: u16) -> i32 {
    let value = (value & 0x07FF) as i32;
    (value << 21) >> 21
}

#[cfg(test)]
mod tests {
    use super::decode_thumb;
    use crate::cpu::thumb::{
        ThumbHalfwordTransferKind, ThumbInstruction, ThumbLoadAddressBase,
        ThumbRegisterOffsetTransferKind,
    };

    #[test]
    fn decodes_literal_load() {
        assert_eq!(
            decode_thumb(0x4902).unwrap(),
            ThumbInstruction::LiteralLoad {
                destination: 1,
                offset: 8,
            }
        );
    }

    #[test]
    fn decodes_register_offset_signed_halfword_load() {
        assert_eq!(
            decode_thumb(0x5E88).unwrap(),
            ThumbInstruction::RegisterOffsetTransfer {
                kind: ThumbRegisterOffsetTransferKind::LoadSignedHalfword,
                offset_register: 2,
                base_register: 1,
                destination: 0,
            }
        );
    }

    #[test]
    fn decodes_halfword_immediate_load() {
        assert_eq!(
            decode_thumb(0x8948).unwrap(),
            ThumbInstruction::HalfwordImmediateTransfer {
                kind: ThumbHalfwordTransferKind::Load,
                offset: 10,
                base_register: 1,
                destination: 0,
            }
        );
    }

    #[test]
    fn decodes_pc_relative_address() {
        assert_eq!(
            decode_thumb(0xA204).unwrap(),
            ThumbInstruction::LoadAddress {
                base: ThumbLoadAddressBase::ProgramCounter,
                destination: 2,
                offset: 16,
            }
        );
    }

    #[test]
    fn decodes_push_with_lr() {
        assert_eq!(
            decode_thumb(0xB503).unwrap(),
            ThumbInstruction::Push {
                registers: 0x03,
                include_link_register: true,
            }
        );
    }

    #[test]
    fn decodes_pop_with_pc() {
        assert_eq!(
            decode_thumb(0xBD01).unwrap(),
            ThumbInstruction::Pop {
                registers: 0x01,
                include_program_counter: true,
            }
        );
    }

    #[test]
    fn decodes_alu_multiply() {
        assert_eq!(
            decode_thumb(0x4348).unwrap(),
            ThumbInstruction::Alu {
                operation: crate::cpu::thumb::ThumbAluOperation::Multiply,
                source: 1,
                destination: 0,
            }
        );
    }

    #[test]
    fn decodes_first_half_of_bl() {
        assert_eq!(
            decode_thumb(0xF001).unwrap(),
            ThumbInstruction::LongBranchWithLink {
                half: crate::cpu::thumb::ThumbLongBranchHalf::First,
                offset: 0x1000,
            }
        );
    }

    #[test]
    fn decodes_second_half_of_bl() {
        assert_eq!(
            decode_thumb(0xF802).unwrap(),
            ThumbInstruction::LongBranchWithLink {
                half: crate::cpu::thumb::ThumbLongBranchHalf::Second,
                offset: 4,
            }
        );
    }
}
