use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfwordTransferKind {
    StoreHalfword,
    LoadHalfword,
    LoadSignedByte,
    LoadSignedHalfword,
}

impl HalfwordTransferKind {
    pub const fn is_load(self) -> bool {
        !matches!(self, Self::StoreHalfword)
    }

    pub const fn uses_halfword(self) -> bool {
        matches!(
            self,
            Self::StoreHalfword | Self::LoadHalfword | Self::LoadSignedHalfword
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfwordTransferOffset {
    Immediate(u8),
    Register(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfwordDataTransferInstruction {
    pub condition: ArmCondition,
    pub kind: HalfwordTransferKind,

    pub pre_index: bool,
    pub add_offset: bool,
    pub write_back: bool,

    pub rn: u8,
    pub rd: u8,

    pub offset: HalfwordTransferOffset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfwordDataTransferDecodeError {
    InvalidEncoding,
    InvalidOperation,
    InvalidPostIndexWriteBackBit,
    InvalidRegisterOffsetEncoding,
    ProgramCounterAsOffsetRegister,
}

pub fn decode_halfword_data_transfer(
    instruction: u32,
) -> Result<HalfwordDataTransferInstruction, HalfwordDataTransferDecodeError> {
    /*
     * cond 000 P U I W L Rn Rd offset_high 1 S H 1 offset_low
     */
    if instruction & 0x0E00_0090 != 0x0000_0090 {
        return Err(HalfwordDataTransferDecodeError::InvalidEncoding);
    }

    let pre_index = instruction & (1 << 24) != 0;
    let add_offset = instruction & (1 << 23) != 0;
    let immediate_offset = instruction & (1 << 22) != 0;
    let w_bit = instruction & (1 << 21) != 0;
    let load = instruction & (1 << 20) != 0;

    let rn = ((instruction >> 16) & 0x0F) as u8;
    let rd = ((instruction >> 12) & 0x0F) as u8;

    let s = instruction & (1 << 6) != 0;
    let h = instruction & (1 << 5) != 0;

    /*
     * For post-indexed forms, W must be zero and write-back is
     * implicit.
     */
    if !pre_index && w_bit {
        return Err(HalfwordDataTransferDecodeError::InvalidPostIndexWriteBackBit);
    }

    let write_back = !pre_index || w_bit;

    let kind = match (load, s, h) {
        (false, false, true) => HalfwordTransferKind::StoreHalfword,

        (true, false, true) => HalfwordTransferKind::LoadHalfword,

        (true, true, false) => HalfwordTransferKind::LoadSignedByte,

        (true, true, true) => HalfwordTransferKind::LoadSignedHalfword,

        _ => {
            return Err(HalfwordDataTransferDecodeError::InvalidOperation);
        }
    };

    let offset = if immediate_offset {
        let high = ((instruction >> 8) & 0x0F) as u8;
        let low = (instruction & 0x0F) as u8;

        HalfwordTransferOffset::Immediate((high << 4) | low)
    } else {
        /*
         * Register-offset form requires bits 11..8 to be zero.
         */
        if instruction & 0x0000_0F00 != 0 {
            return Err(HalfwordDataTransferDecodeError::InvalidRegisterOffsetEncoding);
        }

        let rm = (instruction & 0x0F) as u8;

        if rm == 15 {
            return Err(HalfwordDataTransferDecodeError::ProgramCounterAsOffsetRegister);
        }

        HalfwordTransferOffset::Register(rm)
    };

    Ok(HalfwordDataTransferInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        kind,
        pre_index,
        add_offset,
        write_back,
        rn,
        rd,
        offset,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        HalfwordDataTransferDecodeError, HalfwordTransferKind, HalfwordTransferOffset,
        decode_halfword_data_transfer,
    };

    #[test]
    fn decodes_strh_immediate() {
        // STRH R1, [R0, #4]
        let decoded = decode_halfword_data_transfer(0xE1C0_10B4).unwrap();

        assert_eq!(decoded.offset, HalfwordTransferOffset::Immediate(4));
    }

    #[test]
    fn decodes_ldr_halfword() {
        // LDRH R1, [R0]
        let decoded = decode_halfword_data_transfer(0xE1D0_10B0).unwrap();

        assert_eq!(decoded.kind, HalfwordTransferKind::LoadHalfword);
    }

    #[test]
    fn decodes_ldr_signed_byte() {
        // LDRSB R1, [R0]
        let decoded = decode_halfword_data_transfer(0xE1D0_10D0).unwrap();

        assert_eq!(decoded.kind, HalfwordTransferKind::LoadSignedByte);
    }

    #[test]
    fn decodes_ldr_signed_halfword() {
        // LDRSH R1, [R0]
        let decoded = decode_halfword_data_transfer(0xE1D0_10F0).unwrap();

        assert_eq!(decoded.kind, HalfwordTransferKind::LoadSignedHalfword);
    }

    #[test]
    fn decodes_register_offset() {
        /*
         * LDRH R1, [R0, R2]
         */
        let decoded = decode_halfword_data_transfer(0xE190_10B2).unwrap();

        assert_eq!(decoded.offset, HalfwordTransferOffset::Register(2));
    }

    #[test]
    fn decodes_negative_offset() {
        // LDRH R1, [R0, #-4]
        let decoded = decode_halfword_data_transfer(0xE150_10B4).unwrap();

        assert!(!decoded.add_offset);
    }

    #[test]
    fn decodes_pre_index_writeback() {
        // LDRH R1, [R0, #4]!
        let decoded = decode_halfword_data_transfer(0xE1F0_10B4).unwrap();

        assert!(decoded.pre_index);
        assert!(decoded.write_back);
    }

    #[test]
    fn decodes_post_index() {
        // LDRH R1, [R0], #4
        let decoded = decode_halfword_data_transfer(0xE0D0_10B4).unwrap();

        assert!(!decoded.pre_index);
        assert!(decoded.write_back);
    }

    #[test]
    fn rejects_signed_store_encoding() {
        let result = decode_halfword_data_transfer(0xE1C0_10D0);

        assert_eq!(
            result,
            Err(HalfwordDataTransferDecodeError::InvalidOperation)
        );
    }

    #[test]
    fn rejects_pc_as_register_offset() {
        let result = decode_halfword_data_transfer(0xE190_10BF);

        assert_eq!(
            result,
            Err(HalfwordDataTransferDecodeError::ProgramCounterAsOffsetRegister)
        );
    }
}
