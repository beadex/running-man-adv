use super::{
    ArmInstruction, ArmInstructionKind, BranchDecodeError, BranchExchangeDecodeError,
    DataProcessingDecodeError, classify, condition, decode_branch, decode_branch_exchange,
    decode_data_processing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmDecodeError {
    DataProcessing(DataProcessingDecodeError),
    Branch(BranchDecodeError),
    BranchExchange(BranchExchangeDecodeError),
}

pub fn decode_arm(instruction: u32) -> Result<ArmInstruction, ArmDecodeError> {
    let condition = condition(instruction);
    let kind = classify(instruction);

    match kind {
        ArmInstructionKind::DataProcessing => {
            let decoded =
                decode_data_processing(instruction).map_err(ArmDecodeError::DataProcessing)?;

            Ok(ArmInstruction::DataProcessing(decoded))
        }

        ArmInstructionKind::Branch => {
            let decoded = decode_branch(instruction).map_err(ArmDecodeError::Branch)?;

            Ok(ArmInstruction::Branch(decoded))
        }

        ArmInstructionKind::BranchExchange => {
            let decoded =
                decode_branch_exchange(instruction).map_err(ArmDecodeError::BranchExchange)?;

            Ok(ArmInstruction::BranchExchange(decoded))
        }

        ArmInstructionKind::Multiply => Ok(ArmInstruction::Multiply {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::MultiplyLong => Ok(ArmInstruction::MultiplyLong {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::SingleDataSwap => Ok(ArmInstruction::SingleDataSwap {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::HalfwordDataTransfer => Ok(ArmInstruction::HalfwordDataTransfer {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::SingleDataTransfer => Ok(ArmInstruction::SingleDataTransfer {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::BlockDataTransfer => Ok(ArmInstruction::BlockDataTransfer {
            condition,
            raw: instruction,
        }),

        ArmInstructionKind::CoprocessorDataTransfer => {
            Ok(ArmInstruction::CoprocessorDataTransfer {
                condition,
                raw: instruction,
            })
        }

        ArmInstructionKind::CoprocessorDataOperation => {
            Ok(ArmInstruction::CoprocessorDataOperation {
                condition,
                raw: instruction,
            })
        }

        ArmInstructionKind::CoprocessorRegisterTransfer => {
            Ok(ArmInstruction::CoprocessorRegisterTransfer {
                condition,
                raw: instruction,
            })
        }

        ArmInstructionKind::SoftwareInterrupt => Ok(ArmInstruction::SoftwareInterrupt {
            condition,
            comment: instruction & 0x00FF_FFFF,
        }),

        ArmInstructionKind::Undefined => Ok(ArmInstruction::Undefined {
            condition,
            raw: instruction,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{ArmInstruction, decode_arm};

    use crate::cpu::arm::{ArmCondition, BranchInstruction, DataProcessingOpcode, Operand2};

    #[test]
    fn decodes_data_processing_instruction() {
        // MOV R0, #1
        let instruction = decode_arm(0xE3A0_0001).unwrap();

        match instruction {
            ArmInstruction::DataProcessing(decoded) => {
                assert_eq!(decoded.condition, ArmCondition::Always);

                assert_eq!(decoded.opcode, DataProcessingOpcode::Mov);

                assert_eq!(decoded.rd, 0);

                assert_eq!(
                    decoded.operand2,
                    Operand2::Immediate {
                        value: 1,
                        rotate: 0,
                    }
                );
            }

            other => {
                panic!("expected data-processing instruction, got {other:?}");
            }
        }
    }

    #[test]
    fn decodes_typed_branch_exchange() {
        let instruction = decode_arm(0xE12F_FF1E).unwrap();

        assert_eq!(
            instruction,
            ArmInstruction::BranchExchange(crate::cpu::arm::BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: 14,
            })
        );
    }

    #[test]
    fn decodes_software_interrupt_comment() {
        let instruction = decode_arm(0xEF12_3456).unwrap();

        assert_eq!(
            instruction,
            ArmInstruction::SoftwareInterrupt {
                condition: ArmCondition::Always,
                comment: 0x0012_3456,
            }
        );
    }

    #[test]
    fn preserves_condition_for_non_data_processing_instruction() {
        // BNE
        let instruction = decode_arm(0x1A00_0000).unwrap();

        assert_eq!(instruction.condition(), ArmCondition::NotEqual);
    }

    #[test]
    fn returns_undefined_instruction() {
        /*
         * Major group 011 with bit 4 set.
         */
        let raw = 0xE791_0012;

        let instruction = decode_arm(raw).unwrap();

        assert_eq!(
            instruction,
            ArmInstruction::Undefined {
                condition: ArmCondition::Always,
                raw,
            }
        );
    }

    #[test]
    fn decodes_typed_branch_instruction() {
        // BL with displacement +4.
        let instruction = decode_arm(0xEB00_0001).unwrap();

        assert_eq!(
            instruction,
            ArmInstruction::Branch(BranchInstruction {
                condition: ArmCondition::Always,
                link: true,
                offset: 4,
            })
        );
    }
}
