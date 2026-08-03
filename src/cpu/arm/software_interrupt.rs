use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareInterruptInstruction {
    pub condition: ArmCondition,
    pub comment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareInterruptDecodeError {
    InvalidEncoding,
}

pub const fn decode_software_interrupt(
    instruction: u32,
) -> Result<SoftwareInterruptInstruction, SoftwareInterruptDecodeError> {
    /*
     * cond 1111 imm24
     */
    if instruction & 0x0F00_0000 != 0x0F00_0000 {
        return Err(SoftwareInterruptDecodeError::InvalidEncoding);
    }

    Ok(SoftwareInterruptInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),

        comment: instruction & 0x00FF_FFFF,
    })
}

#[cfg(test)]
mod tests {
    use super::{SoftwareInterruptInstruction, decode_software_interrupt};

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_software_interrupt() {
        assert_eq!(
            decode_software_interrupt(0xEF12_3456,),
            Ok(SoftwareInterruptInstruction {
                condition: ArmCondition::Always,

                comment: 0x0012_3456,
            })
        );
    }
}
