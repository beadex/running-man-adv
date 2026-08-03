use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplyInstruction {
    pub condition: ArmCondition,

    /// false: MUL
    /// true: MLA
    pub accumulate: bool,

    pub set_flags: bool,

    pub rd: u8,
    pub rn: u8,
    pub rs: u8,
    pub rm: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiplyDecodeError {
    InvalidEncoding,

    /// MUL requires the unused Rn field to be zero.
    NonZeroAccumulateRegisterForMul,

    /// ARM7TDMI does not permit R15 in MUL/MLA.
    ProgramCounterUsed,

    /// ARM7TDMI requires Rd and Rm to be different.
    DestinationEqualsMultiplier,
}

pub fn decode_multiply(instruction: u32) -> Result<MultiplyInstruction, MultiplyDecodeError> {
    /*
     * MUL / MLA encoding:
     *
     * 31       28 27       22 21 20 19  16 15  12 11   8 7    4 3   0
     * +----------+-----------+--+--+------+------+------+-------+-----+
     * |   cond   | 000000    | A| S|  Rd  |  Rn  |  Rs  | 1001 | Rm  |
     * +----------+-----------+--+--+------+------+------+-------+-----+
     */
    if instruction & 0x0FC0_00F0 != 0x0000_0090 {
        return Err(MultiplyDecodeError::InvalidEncoding);
    }

    let accumulate = instruction & (1 << 21) != 0;
    let set_flags = instruction & (1 << 20) != 0;

    let rd = ((instruction >> 16) & 0x0F) as u8;
    let rn = ((instruction >> 12) & 0x0F) as u8;
    let rs = ((instruction >> 8) & 0x0F) as u8;
    let rm = (instruction & 0x0F) as u8;

    if !accumulate && rn != 0 {
        return Err(MultiplyDecodeError::NonZeroAccumulateRegisterForMul);
    }

    if rd == 15 || rn == 15 || rs == 15 || rm == 15 {
        return Err(MultiplyDecodeError::ProgramCounterUsed);
    }

    if rd == rm {
        return Err(MultiplyDecodeError::DestinationEqualsMultiplier);
    }

    Ok(MultiplyInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        accumulate,
        set_flags,
        rd,
        rn,
        rs,
        rm,
    })
}

#[cfg(test)]
mod tests {
    use super::{MultiplyDecodeError, MultiplyInstruction, decode_multiply};

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_mul() {
        /*
         * MUL R0, R1, R2
         *
         * Rd = R0
         * Rm = R1
         * Rs = R2
         */
        assert_eq!(
            decode_multiply(0xE000_0291),
            Ok(MultiplyInstruction {
                condition: ArmCondition::Always,
                accumulate: false,
                set_flags: false,
                rd: 0,
                rn: 0,
                rs: 2,
                rm: 1,
            })
        );
    }

    #[test]
    fn decodes_muls() {
        // MULS R0, R1, R2
        let decoded = decode_multiply(0xE010_0291).unwrap();

        assert!(!decoded.accumulate);
        assert!(decoded.set_flags);
    }

    #[test]
    fn decodes_mla() {
        /*
         * MLA R0, R1, R2, R3
         *
         * R0 = R1 * R2 + R3
         */
        assert_eq!(
            decode_multiply(0xE020_3291),
            Ok(MultiplyInstruction {
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
    fn decodes_mlas() {
        // MLAS R0, R1, R2, R3
        let decoded = decode_multiply(0xE030_3291).unwrap();

        assert!(decoded.accumulate);
        assert!(decoded.set_flags);
    }

    #[test]
    fn preserves_condition() {
        // MULNE R0, R1, R2
        let decoded = decode_multiply(0x1000_0291).unwrap();

        assert_eq!(decoded.condition, ArmCondition::NotEqual);
    }

    #[test]
    fn rejects_non_multiply_encoding() {
        assert_eq!(
            decode_multiply(0xE1A0_0000),
            Err(MultiplyDecodeError::InvalidEncoding)
        );
    }

    #[test]
    fn rejects_non_zero_rn_for_mul() {
        /*
         * A=0 means MUL, so Rn is an unused field and must be zero.
         */
        assert_eq!(
            decode_multiply(0xE000_1291),
            Err(MultiplyDecodeError::NonZeroAccumulateRegisterForMul)
        );
    }

    #[test]
    fn rejects_program_counter() {
        // MUL R0, PC, R2
        assert_eq!(
            decode_multiply(0xE000_029F),
            Err(MultiplyDecodeError::ProgramCounterUsed)
        );
    }

    #[test]
    fn rejects_rd_equal_to_rm() {
        // MUL R1, R1, R2
        assert_eq!(
            decode_multiply(0xE001_0291),
            Err(MultiplyDecodeError::DestinationEqualsMultiplier)
        );
    }
}
