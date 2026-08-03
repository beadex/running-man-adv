use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplyLongInstruction {
    pub condition: ArmCondition,

    /// U bit:
    ///
    /// false -> unsigned: UMULL / UMLAL
    /// true  -> signed:   SMULL / SMLAL
    pub signed: bool,

    /// A bit:
    ///
    /// false -> MULL
    /// true  -> MLAL
    pub accumulate: bool,

    /// S bit.
    pub set_flags: bool,

    pub rd_hi: u8,
    pub rd_lo: u8,
    pub rs: u8,
    pub rm: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiplyLongDecodeError {
    InvalidEncoding,
    ProgramCounterUsed,
    DestinationRegistersEqual,
}

pub fn decode_multiply_long(
    instruction: u32,
) -> Result<MultiplyLongInstruction, MultiplyLongDecodeError> {
    /*
     * Multiply Long encoding:
     *
     * 31      28 27    23 22 21 20 19  16 15  12 11   8 7    4 3   0
     * +---------+--------+--+--+--+------+------+------+-------+-----+
     * |  cond   | 00001  | U| A| S| RdHi | RdLo |  Rs  | 1001 | Rm  |
     * +---------+--------+--+--+--+------+------+------+-------+-----+
     */
    if instruction & 0x0F80_00F0 != 0x0080_0090 {
        return Err(MultiplyLongDecodeError::InvalidEncoding);
    }

    let signed = instruction & (1 << 22) != 0;
    let accumulate = instruction & (1 << 21) != 0;
    let set_flags = instruction & (1 << 20) != 0;

    let rd_hi = ((instruction >> 16) & 0x0F) as u8;
    let rd_lo = ((instruction >> 12) & 0x0F) as u8;
    let rs = ((instruction >> 8) & 0x0F) as u8;
    let rm = (instruction & 0x0F) as u8;

    if rd_hi == 15 || rd_lo == 15 || rs == 15 || rm == 15 {
        return Err(MultiplyLongDecodeError::ProgramCounterUsed);
    }

    if rd_hi == rd_lo {
        return Err(MultiplyLongDecodeError::DestinationRegistersEqual);
    }

    Ok(MultiplyLongInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        signed,
        accumulate,
        set_flags,
        rd_hi,
        rd_lo,
        rs,
        rm,
    })
}

#[cfg(test)]
mod tests {
    use super::{MultiplyLongDecodeError, MultiplyLongInstruction, decode_multiply_long};

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_umull() {
        /*
         * UMULL R0, R1, R2, R3
         *
         * RdLo = R0
         * RdHi = R1
         * Rm   = R2
         * Rs   = R3
         */
        assert_eq!(
            decode_multiply_long(0xE081_0392),
            Ok(MultiplyLongInstruction {
                condition: ArmCondition::Always,
                signed: false,
                accumulate: false,
                set_flags: false,
                rd_hi: 1,
                rd_lo: 0,
                rs: 3,
                rm: 2,
            })
        );
    }

    #[test]
    fn decodes_umlal() {
        let decoded = decode_multiply_long(0xE0A1_0392).unwrap();

        assert!(!decoded.signed);
        assert!(decoded.accumulate);
        assert!(!decoded.set_flags);
    }

    #[test]
    fn decodes_smull() {
        let decoded = decode_multiply_long(0xE0C1_0392).unwrap();

        assert!(decoded.signed);
        assert!(!decoded.accumulate);
        assert!(!decoded.set_flags);
    }

    #[test]
    fn decodes_smlal() {
        let decoded = decode_multiply_long(0xE0E1_0392).unwrap();

        assert!(decoded.signed);
        assert!(decoded.accumulate);
    }

    #[test]
    fn decodes_flag_setting_variant() {
        // UMULLS R0, R1, R2, R3
        let decoded = decode_multiply_long(0xE091_0392).unwrap();

        assert!(decoded.set_flags);
    }

    #[test]
    fn preserves_condition() {
        // UMULLNE R0, R1, R2, R3
        let decoded = decode_multiply_long(0x1081_0392).unwrap();

        assert_eq!(decoded.condition, ArmCondition::NotEqual);
    }

    #[test]
    fn rejects_pc_usage() {
        /*
         * UMULL R0, R1, PC, R3
         */
        assert_eq!(
            decode_multiply_long(0xE081_039F),
            Err(MultiplyLongDecodeError::ProgramCounterUsed)
        );
    }

    #[test]
    fn rejects_equal_destination_registers() {
        /*
         * RdLo = R1
         * RdHi = R1
         */
        assert_eq!(
            decode_multiply_long(0xE081_1392),
            Err(MultiplyLongDecodeError::DestinationRegistersEqual)
        );
    }

    #[test]
    fn rejects_non_multiply_long_encoding() {
        assert_eq!(
            decode_multiply_long(0xE1A0_0000),
            Err(MultiplyLongDecodeError::InvalidEncoding)
        );
    }
}
