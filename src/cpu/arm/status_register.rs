use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStatusRegister {
    Cpsr,
    Spsr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusRegisterMask {
    pub control: bool,
    pub extension: bool,
    pub status: bool,
    pub flags: bool,
}

impl StatusRegisterMask {
    pub const fn from_bits(bits: u8) -> Self {
        Self {
            control: bits & 0b0001 != 0,
            extension: bits & 0b0010 != 0,
            status: bits & 0b0100 != 0,
            flags: bits & 0b1000 != 0,
        }
    }

    pub const fn bit_mask(self) -> u32 {
        let mut mask = 0;

        if self.control {
            mask |= 0x0000_00FF;
        }

        if self.extension {
            mask |= 0x0000_FF00;
        }

        if self.status {
            mask |= 0x00FF_0000;
        }

        if self.flags {
            mask |= 0xFF00_0000;
        }

        mask
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusRegisterOperand {
    Register(u8),

    Immediate { value: u8, rotate: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusRegisterInstruction {
    Mrs {
        condition: ArmCondition,
        source: ProgramStatusRegister,
        rd: u8,
    },

    Msr {
        condition: ArmCondition,
        destination: ProgramStatusRegister,
        mask: StatusRegisterMask,
        operand: StatusRegisterOperand,
    },
}

impl StatusRegisterInstruction {
    pub const fn condition(self) -> ArmCondition {
        match self {
            Self::Mrs { condition, .. } | Self::Msr { condition, .. } => condition,
        }
    }
}

pub fn decode_status_register(
    instruction: u32,
) -> Result<StatusRegisterInstruction, StatusRegisterDecodeError> {
    if instruction & 0x0FBF_0FFF == 0x010F_0000 {
        return decode_mrs(instruction);
    }

    if instruction & 0x0FB0_FFF0 == 0x0120_F000 {
        return decode_msr_register(instruction);
    }

    if instruction & 0x0FB0_F000 == 0x0320_F000 {
        return decode_msr_immediate(instruction);
    }

    Err(StatusRegisterDecodeError::InvalidEncoding)
}

fn decode_mrs(instruction: u32) -> Result<StatusRegisterInstruction, StatusRegisterDecodeError> {
    let rd = ((instruction >> 12) & 0x0F) as u8;

    if rd == 15 {
        return Err(StatusRegisterDecodeError::ProgramCounterUsed);
    }

    Ok(StatusRegisterInstruction::Mrs {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),

        source: decode_psr_select(instruction),
        rd,
    })
}

const fn decode_psr_select(instruction: u32) -> ProgramStatusRegister {
    if instruction & (1 << 22) != 0 {
        ProgramStatusRegister::Spsr
    } else {
        ProgramStatusRegister::Cpsr
    }
}

fn decode_msr_register(
    instruction: u32,
) -> Result<StatusRegisterInstruction, StatusRegisterDecodeError> {
    let rm = (instruction & 0x0F) as u8;

    if rm == 15 {
        return Err(StatusRegisterDecodeError::ProgramCounterUsed);
    }

    decode_msr_common(instruction, StatusRegisterOperand::Register(rm))
}

fn decode_msr_immediate(
    instruction: u32,
) -> Result<StatusRegisterInstruction, StatusRegisterDecodeError> {
    decode_msr_common(
        instruction,
        StatusRegisterOperand::Immediate {
            value: (instruction & 0xFF) as u8,
            rotate: ((instruction >> 8) & 0x0F) as u8,
        },
    )
}

fn decode_msr_common(
    instruction: u32,
    operand: StatusRegisterOperand,
) -> Result<StatusRegisterInstruction, StatusRegisterDecodeError> {
    let field_bits = ((instruction >> 16) & 0x0F) as u8;

    if field_bits == 0 {
        return Err(StatusRegisterDecodeError::EmptyFieldMask);
    }

    Ok(StatusRegisterInstruction::Msr {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),

        destination: decode_psr_select(instruction),

        mask: StatusRegisterMask::from_bits(field_bits),

        operand,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusRegisterDecodeError {
    InvalidEncoding,
    ProgramCounterUsed,
    EmptyFieldMask,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_mrs_cpsr() {
        // MRS R0, CPSR
        assert_eq!(
            decode_status_register(0xE10F_0000),
            Ok(StatusRegisterInstruction::Mrs {
                condition: ArmCondition::Always,
                source: ProgramStatusRegister::Cpsr,
                rd: 0,
            })
        );
    }

    #[test]
    fn decodes_mrs_spsr() {
        // MRS R1, SPSR
        assert_eq!(
            decode_status_register(0xE14F_1000),
            Ok(StatusRegisterInstruction::Mrs {
                condition: ArmCondition::Always,
                source: ProgramStatusRegister::Spsr,
                rd: 1,
            })
        );
    }

    #[test]
    fn decodes_msr_cpsr_register() {
        // MSR CPSR_fc, R0
        let decoded = decode_status_register(0xE129_F000).unwrap();

        assert_eq!(
            decoded,
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
            }
        );
    }

    #[test]
    fn decodes_msr_immediate() {
        // MSR CPSR_f, #0x80000000
        let decoded = decode_status_register(0xE328_F102).unwrap();

        match decoded {
            StatusRegisterInstruction::Msr {
                destination,
                mask,
                operand,
                ..
            } => {
                assert_eq!(destination, ProgramStatusRegister::Cpsr);

                assert!(mask.flags);
                assert!(!mask.control);

                assert_eq!(
                    operand,
                    StatusRegisterOperand::Immediate {
                        value: 2,
                        rotate: 1,
                    }
                );
            }

            _ => panic!("expected MSR"),
        }
    }

    #[test]
    fn rejects_pc_operand() {
        assert_eq!(
            decode_status_register(0xE129_F00F),
            Err(StatusRegisterDecodeError::ProgramCounterUsed)
        );
    }
}
