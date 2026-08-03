use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchInstruction {
    pub condition: ArmCondition,

    /// `true` for BL, `false` for B.
    pub link: bool,

    /// Signed byte displacement after the encoded imm24 has been
    /// sign-extended and shifted left by two.
    pub offset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchDecodeError {
    InvalidEncoding,
}

pub const fn decode_branch(instruction: u32) -> Result<BranchInstruction, BranchDecodeError> {
    /*
     * Branch encoding:
     *
     * 31        28 27  25 24 23                       0
     * +-----------+------+--+--------------------------+
     * | condition | 101  | L|          imm24           |
     * +-----------+------+--+--------------------------+
     */
    if instruction & 0x0E00_0000 != 0x0A00_0000 {
        return Err(BranchDecodeError::InvalidEncoding);
    }

    let condition = ArmCondition::from_bits((instruction >> 28) as u8);

    let link = instruction & (1 << 24) != 0;
    let immediate = instruction & 0x00FF_FFFF;

    /*
     * Sign-extend the 24-bit displacement after appending two
     * low zero bits.
     *
     * Moving the value into the high 26 bits and applying an
     * arithmetic right shift produces the signed 26-bit result.
     */
    let signed_immediate = ((immediate << 8) as i32) >> 8;

    let offset = signed_immediate << 2;

    Ok(BranchInstruction {
        condition,
        link,
        offset,
    })
}

#[cfg(test)]
mod tests {
    use super::{BranchDecodeError, BranchInstruction, decode_branch};

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_branch_without_link() {
        // B +0
        assert_eq!(
            decode_branch(0xEA00_0000),
            Ok(BranchInstruction {
                condition: ArmCondition::Always,
                link: false,
                offset: 0,
            })
        );
    }

    #[test]
    fn decodes_branch_with_link() {
        // BL +0
        assert_eq!(
            decode_branch(0xEB00_0000),
            Ok(BranchInstruction {
                condition: ArmCondition::Always,
                link: true,
                offset: 0,
            })
        );
    }

    #[test]
    fn decodes_positive_branch_offset() {
        /*
         * imm24 = 1
         * displacement = 1 << 2 = 4
         */
        assert_eq!(decode_branch(0xEA00_0001).unwrap().offset, 4);
    }

    #[test]
    fn decodes_larger_positive_branch_offset() {
        /*
         * imm24 = 0x10
         * displacement = 0x40
         */
        assert_eq!(decode_branch(0xEA00_0010).unwrap().offset, 0x40);
    }

    #[test]
    fn sign_extends_negative_branch_offset() {
        /*
         * imm24 = 0xFFFFFF
         *
         * signed imm24 = -1
         * displacement = -4
         */
        assert_eq!(decode_branch(0xEAFF_FFFF).unwrap().offset, -4);
    }

    #[test]
    fn decodes_branch_back_by_eight_bytes() {
        /*
         * imm24 = 0xFFFFFE
         *
         * -2 << 2 = -8
         */
        assert_eq!(decode_branch(0xEAFF_FFFE).unwrap().offset, -8);
    }

    #[test]
    fn decodes_minimum_branch_offset() {
        /*
         * Signed imm24 minimum:
         *
         * -0x800000 << 2 = -0x02000000
         */
        assert_eq!(decode_branch(0xEA80_0000).unwrap().offset, -0x0200_0000);
    }

    #[test]
    fn decodes_maximum_branch_offset() {
        /*
         * Signed imm24 maximum:
         *
         * 0x7FFFFF << 2 = 0x01FFFFFC
         */
        assert_eq!(decode_branch(0xEA7F_FFFF).unwrap().offset, 0x01FF_FFFC);
    }

    #[test]
    fn preserves_condition() {
        // BNE +0
        let decoded = decode_branch(0x1A00_0000).unwrap();

        assert_eq!(decoded.condition, ArmCondition::NotEqual);
    }

    #[test]
    fn rejects_non_branch_instruction() {
        // MOV R0, R0
        assert_eq!(
            decode_branch(0xE1A0_0000),
            Err(BranchDecodeError::InvalidEncoding)
        );
    }
}
