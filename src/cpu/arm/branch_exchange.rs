use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchExchangeInstruction {
    pub condition: ArmCondition,
    pub register: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchExchangeDecodeError {
    InvalidEncoding,
}

pub const fn decode_branch_exchange(
    instruction: u32,
) -> Result<BranchExchangeInstruction, BranchExchangeDecodeError> {
    /*
     * BX encoding:
     *
     * cond 0001 0010 1111 1111 1111 0001 Rm
     */
    if instruction & 0x0FFF_FFF0 != 0x012F_FF10 {
        return Err(BranchExchangeDecodeError::InvalidEncoding);
    }

    Ok(BranchExchangeInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        register: (instruction & 0x0F) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::{BranchExchangeDecodeError, BranchExchangeInstruction, decode_branch_exchange};

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_bx_lr() {
        assert_eq!(
            decode_branch_exchange(0xE12F_FF1E),
            Ok(BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: 14,
            })
        );
    }

    #[test]
    fn decodes_bx_r0() {
        assert_eq!(
            decode_branch_exchange(0xE12F_FF10),
            Ok(BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: 0,
            })
        );
    }

    #[test]
    fn preserves_condition() {
        assert_eq!(
            decode_branch_exchange(0x112F_FF10).unwrap().condition,
            ArmCondition::NotEqual
        );
    }

    #[test]
    fn rejects_non_bx_instruction() {
        assert_eq!(
            decode_branch_exchange(0xE1A0_0000),
            Err(BranchExchangeDecodeError::InvalidEncoding)
        );
    }
}
