use super::{
    ArmInstruction, ArmInstructionKind, BlockDataTransferDecodeError, BranchDecodeError,
    BranchExchangeDecodeError, DataProcessingDecodeError, HalfwordDataTransferDecodeError,
    MultiplyDecodeError, MultiplyLongDecodeError, SingleDataTransferDecodeError,
    SoftwareInterruptDecodeError, StatusRegisterDecodeError, classify, condition,
    decode_block_data_transfer, decode_branch, decode_branch_exchange, decode_data_processing,
    decode_halfword_data_transfer, decode_multiply, decode_multiply_long,
    decode_single_data_transfer, decode_software_interrupt, decode_status_register,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmDecodeError {
    BlockDataTransfer(BlockDataTransferDecodeError),
    Branch(BranchDecodeError),
    BranchExchange(BranchExchangeDecodeError),
    DataProcessing(DataProcessingDecodeError),
    HalfwordDataTransfer(HalfwordDataTransferDecodeError),
    Multiply(MultiplyDecodeError),
    MultiplyLong(MultiplyLongDecodeError),
    SingleDataTransfer(SingleDataTransferDecodeError),
    SoftwareInterrupt(SoftwareInterruptDecodeError),
    StatusRegister(StatusRegisterDecodeError),
}

pub fn decode_arm(instruction: u32) -> Result<ArmInstruction, ArmDecodeError> {
    let condition = condition(instruction);
    let kind = classify(instruction);

    match kind {
        ArmInstructionKind::BlockDataTransfer => {
            let decoded = decode_block_data_transfer(instruction)
                .map_err(ArmDecodeError::BlockDataTransfer)?;

            Ok(ArmInstruction::BlockDataTransfer(decoded))
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

        ArmInstructionKind::DataProcessing => {
            let decoded =
                decode_data_processing(instruction).map_err(ArmDecodeError::DataProcessing)?;

            Ok(ArmInstruction::DataProcessing(decoded))
        }

        ArmInstructionKind::HalfwordDataTransfer => {
            let decoded = decode_halfword_data_transfer(instruction)
                .map_err(ArmDecodeError::HalfwordDataTransfer)?;

            Ok(ArmInstruction::HalfwordDataTransfer(decoded))
        }

        ArmInstructionKind::Multiply => {
            let decoded = decode_multiply(instruction).map_err(ArmDecodeError::Multiply)?;

            Ok(ArmInstruction::Multiply(decoded))
        }

        ArmInstructionKind::MultiplyLong => {
            let decoded =
                decode_multiply_long(instruction).map_err(ArmDecodeError::MultiplyLong)?;

            Ok(ArmInstruction::MultiplyLong(decoded))
        }

        ArmInstructionKind::SingleDataTransfer => {
            let decoded = decode_single_data_transfer(instruction)
                .map_err(ArmDecodeError::SingleDataTransfer)?;

            Ok(ArmInstruction::SingleDataTransfer(decoded))
        }

        ArmInstructionKind::SoftwareInterrupt => {
            let decoded = decode_software_interrupt(instruction)
                .map_err(ArmDecodeError::SoftwareInterrupt)?;

            Ok(ArmInstruction::SoftwareInterrupt(decoded))
        }

        ArmInstructionKind::StatusRegister => {
            let decoded =
                decode_status_register(instruction).map_err(ArmDecodeError::StatusRegister)?;

            Ok(ArmInstruction::StatusRegister(decoded))
        }

        ArmInstructionKind::SingleDataSwap => Ok(ArmInstruction::SingleDataSwap {
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

    #[test]
    fn decodes_typed_mla() {
        let instruction = decode_arm(0xE020_3291).unwrap();

        assert_eq!(
            instruction,
            ArmInstruction::Multiply(crate::cpu::arm::MultiplyInstruction {
                condition: ArmCondition::Always,
                accumulate: true,
                set_flags: false,
                rd: 0,
                rn: 3,
                rs: 2,
                rm: 1,
            })
        );
    }

    #[test]
    fn decodes_typed_block_data_transfer() {
        /*
         * STMDB SP!, {R4-R7, LR}
         */
        let instruction = decode_arm(0xE92D_40F0).unwrap();

        match instruction {
            ArmInstruction::BlockDataTransfer(decoded) => {
                assert!(!decoded.load);
                assert!(decoded.write_back);
                assert_eq!(decoded.rn, 13);

                assert_eq!(
                    decoded.addressing_mode,
                    crate::cpu::arm::BlockAddressingMode::DecrementBefore
                );

                assert!(decoded.registers.contains(14));
            }

            other => {
                panic!("expected block data transfer, got {other:?}");
            }
        }
    }

    #[test]
    fn decodes_typed_signed_multiply_accumulate_long() {
        /*
         * SMLAL R0, R1, R2, R3
         */
        let instruction = decode_arm(0xE0E1_0392).unwrap();

        match instruction {
            ArmInstruction::MultiplyLong(decoded) => {
                assert!(decoded.signed);
                assert!(decoded.accumulate);
                assert_eq!(decoded.rd_lo, 0);
                assert_eq!(decoded.rd_hi, 1);
                assert_eq!(decoded.rm, 2);
                assert_eq!(decoded.rs, 3);
            }

            other => {
                panic!("expected multiply-long instruction, got {other:?}");
            }
        }
    }
}
