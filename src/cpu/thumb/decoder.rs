use super::{
    ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbCondition, ThumbHighRegisterOperation,
    ThumbImmediateOperation, ThumbInstruction, ThumbShiftOperation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbDecodeError {
    UndefinedConditionalBranch,

    UnsupportedInstruction { raw: u16 },
}

pub fn decode_thumb(raw: u16) -> Result<ThumbInstruction, ThumbDecodeError> {
    /*
     * Decoding order matters because several THUMB instruction groups
     * share broad prefixes.
     */

    /*
     * Format 1:
     *
     * 000 op offset5 Rs Rd
     *
     * op=11 belongs to Format 2, not Format 1.
     */
    if raw & 0xE000 == 0x0000 && ((raw >> 11) & 0b11) != 0b11 {
        return Ok(decode_move_shifted_register(raw));
    }

    /*
     * Format 2:
     *
     * 00011 I op operand3 Rs Rd
     */
    if raw & 0xF800 == 0x1800 {
        return Ok(decode_add_subtract(raw));
    }

    /*
     * Format 3:
     *
     * 001 op Rd imm8
     */
    if raw & 0xE000 == 0x2000 {
        return Ok(decode_immediate(raw));
    }

    /*
     * Format 5:
     *
     * 010001 op H1 H2 Rs/Hs Rd/Hd
     */
    if raw & 0xFC00 == 0x4400 {
        return Ok(decode_high_register(raw));
    }

    /*
     * Formats 16 and 17:
     *
     * 1101 condition imm8
     *
     * condition=1111 is SWI.
     * condition=1110 is undefined.
     */
    if raw & 0xF000 == 0xD000 {
        return decode_conditional_or_swi(raw);
    }

    /*
     * Format 18:
     *
     * 11100 imm11
     */
    if raw & 0xF800 == 0xE000 {
        return Ok(decode_unconditional_branch(raw));
    }

    Err(ThumbDecodeError::UnsupportedInstruction { raw })
}

fn decode_move_shifted_register(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 11) & 0b11 {
        0b00 => ThumbShiftOperation::LogicalLeft,

        0b01 => ThumbShiftOperation::LogicalRight,

        0b10 => ThumbShiftOperation::ArithmeticRight,

        _ => {
            unreachable!("format 1 excludes op=11")
        }
    };

    let offset = ((raw >> 6) & 0x1F) as u8;

    let source = ((raw >> 3) & 0x07) as u8;

    let destination = (raw & 0x07) as u8;

    ThumbInstruction::MoveShiftedRegister {
        operation,
        offset,
        source,
        destination,
    }
}

fn decode_add_subtract(raw: u16) -> ThumbInstruction {
    let immediate = raw & (1 << 10) != 0;

    let operation = if raw & (1 << 9) != 0 {
        ThumbAddSubtractOperation::Subtract
    } else {
        ThumbAddSubtractOperation::Add
    };

    let raw_operand = ((raw >> 6) & 0x07) as u8;

    let operand = if immediate {
        ThumbAddSubtractOperand::Immediate(raw_operand)
    } else {
        ThumbAddSubtractOperand::Register(raw_operand)
    };

    let source = ((raw >> 3) & 0x07) as u8;

    let destination = (raw & 0x07) as u8;

    ThumbInstruction::AddSubtract {
        operation,
        operand,
        source,
        destination,
    }
}

fn decode_immediate(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 11) & 0b11 {
        0b00 => ThumbImmediateOperation::Move,

        0b01 => ThumbImmediateOperation::Compare,

        0b10 => ThumbImmediateOperation::Add,

        0b11 => ThumbImmediateOperation::Subtract,

        _ => unreachable!(),
    };

    let destination = ((raw >> 8) & 0x07) as u8;

    let immediate = (raw & 0xFF) as u8;

    ThumbInstruction::Immediate {
        operation,
        destination,
        immediate,
    }
}

fn decode_high_register(raw: u16) -> ThumbInstruction {
    let operation = match (raw >> 8) & 0b11 {
        0b00 => ThumbHighRegisterOperation::Add,

        0b01 => ThumbHighRegisterOperation::Compare,

        0b10 => ThumbHighRegisterOperation::Move,

        0b11 => ThumbHighRegisterOperation::BranchExchange,

        _ => unreachable!(),
    };

    let destination_high = ((raw >> 7) & 1) as u8;

    let source_high = ((raw >> 6) & 1) as u8;

    let source = (((raw >> 3) & 0x07) as u8) | (source_high << 3);

    let destination = ((raw & 0x07) as u8) | (destination_high << 3);

    ThumbInstruction::HighRegister {
        operation,
        source,
        destination,
    }
}

fn decode_conditional_or_swi(raw: u16) -> Result<ThumbInstruction, ThumbDecodeError> {
    let condition_bits = ((raw >> 8) & 0x0F) as u8;

    let immediate = (raw & 0xFF) as u8;

    if condition_bits == 0x0F {
        return Ok(ThumbInstruction::SoftwareInterrupt { comment: immediate });
    }

    if condition_bits == 0x0E {
        return Err(ThumbDecodeError::UndefinedConditionalBranch);
    }

    let condition = decode_condition(condition_bits);

    /*
     * Sign-extend imm8 and multiply by two.
     */
    let offset = ((immediate as i8) as i16) << 1;

    Ok(ThumbInstruction::ConditionalBranch { condition, offset })
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

        _ => {
            unreachable!("condition E/F handled separately")
        }
    }
}

fn decode_unconditional_branch(raw: u16) -> ThumbInstruction {
    let immediate = (raw & 0x07FF) as i32;

    /*
     * Sign-extend 11-bit immediate before multiplying by two.
     */
    let signed = (immediate << 21) >> 21;

    let offset = signed << 1;

    ThumbInstruction::UnconditionalBranch { offset }
}

#[cfg(test)]
mod tests {
    use super::{ThumbDecodeError, decode_thumb};

    use crate::cpu::thumb::{
        ThumbAddSubtractOperand, ThumbAddSubtractOperation, ThumbCondition,
        ThumbHighRegisterOperation, ThumbImmediateOperation, ThumbInstruction, ThumbShiftOperation,
    };

    #[test]
    fn decodes_lsl_immediate() {
        /*
         * LSL R1, R0, #1
         */
        assert_eq!(
            decode_thumb(0x0041),
            Ok(ThumbInstruction::MoveShiftedRegister {
                operation: ThumbShiftOperation::LogicalLeft,

                offset: 1,
                source: 0,
                destination: 1,
            },),
        );
    }

    #[test]
    fn decodes_add_register() {
        /*
         * ADD R0, R1, R2
         */
        assert_eq!(
            decode_thumb(0x1888),
            Ok(ThumbInstruction::AddSubtract {
                operation: ThumbAddSubtractOperation::Add,

                operand: ThumbAddSubtractOperand::Register(2),

                source: 1,
                destination: 0,
            },),
        );
    }

    #[test]
    fn decodes_subtract_immediate_three() {
        /*
         * SUB R0, R1, #3
         */
        assert_eq!(
            decode_thumb(0x1EC8),
            Ok(ThumbInstruction::AddSubtract {
                operation: ThumbAddSubtractOperation::Subtract,

                operand: ThumbAddSubtractOperand::Immediate(3),

                source: 1,
                destination: 0,
            },),
        );
    }

    #[test]
    fn decodes_mov_immediate() {
        /*
         * MOV R0, #10
         */
        assert_eq!(
            decode_thumb(0x200A),
            Ok(ThumbInstruction::Immediate {
                operation: ThumbImmediateOperation::Move,

                destination: 0,
                immediate: 10,
            },),
        );
    }

    #[test]
    fn decodes_bx() {
        /*
         * BX R0
         */
        assert_eq!(
            decode_thumb(0x4700),
            Ok(ThumbInstruction::HighRegister {
                operation: ThumbHighRegisterOperation::BranchExchange,

                source: 0,
                destination: 0,
            },),
        );
    }

    #[test]
    fn decodes_equal_branch() {
        /*
         * BEQ +4
         *
         * imm8=2, actual offset=4.
         */
        assert_eq!(
            decode_thumb(0xD002),
            Ok(ThumbInstruction::ConditionalBranch {
                condition: ThumbCondition::Equal,

                offset: 4,
            },),
        );
    }

    #[test]
    fn decodes_negative_unconditional_branch() {
        /*
         * B -4
         *
         * offset=-4 means imm11=-2.
         */
        assert_eq!(
            decode_thumb(0xE7FE),
            Ok(ThumbInstruction::UnconditionalBranch { offset: -4 },),
        );
    }

    #[test]
    fn decodes_swi() {
        assert_eq!(
            decode_thumb(0xDF42),
            Ok(ThumbInstruction::SoftwareInterrupt { comment: 0x42 },),
        );
    }

    #[test]
    fn rejects_condition_e() {
        assert_eq!(
            decode_thumb(0xDE00),
            Err(ThumbDecodeError::UndefinedConditionalBranch,),
        );
    }
}
