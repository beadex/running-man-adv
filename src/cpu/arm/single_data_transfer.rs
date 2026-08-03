use super::{ArmCondition, ShiftType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOffset {
    Immediate(u16),

    ShiftedRegister {
        rm: u8,
        shift_type: ShiftType,
        shift_amount: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleDataTransferInstruction {
    pub condition: ArmCondition,

    /// P bit.
    ///
    /// true  -> apply offset before transfer
    /// false -> transfer using base, then apply offset
    pub pre_index: bool,

    /// U bit.
    ///
    /// true  -> base + offset
    /// false -> base - offset
    pub add_offset: bool,

    /// B bit.
    ///
    /// true  -> byte transfer
    /// false -> word transfer
    pub byte: bool,

    /// Effective write-back.
    ///
    /// Post-indexed transfers always write back.
    pub write_back: bool,

    /// Post-indexed T variant.
    ///
    /// This requires CPU privilege-mode support, so execution will
    /// reject it temporarily.
    pub force_user_mode: bool,

    /// L bit.
    ///
    /// true  -> LDR/LDRB
    /// false -> STR/STRB
    pub load: bool,

    pub rn: u8,
    pub rd: u8,
    pub offset: TransferOffset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleDataTransferDecodeError {
    InvalidEncoding,
    InvalidRegisterOffset,
    ProgramCounterAsOffsetRegister,
}

pub fn decode_single_data_transfer(
    instruction: u32,
) -> Result<SingleDataTransferInstruction, SingleDataTransferDecodeError> {
    /*
     * Single Data Transfer:
     *
     * 31       28 27 26 25 24 23 22 21 20 19  16 15  12 11       0
     * +----------+-----+--+--+--+--+--+--+------+------+-----------+
     * |   cond   | 01  | I| P| U| B| W| L|  Rn  |  Rd  |  offset   |
     * +----------+-----+--+--+--+--+--+--+------+------+-----------+
     */
    if instruction & 0x0C00_0000 != 0x0400_0000 {
        return Err(SingleDataTransferDecodeError::InvalidEncoding);
    }

    let register_offset = instruction & (1 << 25) != 0;
    let pre_index = instruction & (1 << 24) != 0;
    let add_offset = instruction & (1 << 23) != 0;
    let byte = instruction & (1 << 22) != 0;
    let bit_w = instruction & (1 << 21) != 0;
    let load = instruction & (1 << 20) != 0;

    let rn = ((instruction >> 16) & 0x0F) as u8;
    let rd = ((instruction >> 12) & 0x0F) as u8;

    /*
     * Post-indexed transfers always write back.
     *
     * With P=0, bit 21 is T rather than W.
     */
    let write_back = !pre_index || bit_w;
    let force_user_mode = !pre_index && bit_w;

    let offset = if register_offset {
        decode_register_offset(instruction)?
    } else {
        TransferOffset::Immediate((instruction & 0x0FFF) as u16)
    };

    Ok(SingleDataTransferInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        pre_index,
        add_offset,
        byte,
        write_back,
        force_user_mode,
        load,
        rn,
        rd,
        offset,
    })
}

fn decode_register_offset(
    instruction: u32,
) -> Result<TransferOffset, SingleDataTransferDecodeError> {
    /*
     * Register offset only supports shift-by-immediate:
     *
     * bits 11..7 = shift amount
     * bits 6..5  = shift type
     * bit 4      = 0
     * bits 3..0  = Rm
     */
    if instruction & (1 << 4) != 0 {
        return Err(SingleDataTransferDecodeError::InvalidRegisterOffset);
    }

    let rm = (instruction & 0x0F) as u8;

    if rm == 15 {
        return Err(SingleDataTransferDecodeError::ProgramCounterAsOffsetRegister);
    }

    Ok(TransferOffset::ShiftedRegister {
        rm,
        shift_type: ShiftType::from_bits(((instruction >> 5) & 0b11) as u8),
        shift_amount: ((instruction >> 7) & 0x1F) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SingleDataTransferDecodeError, SingleDataTransferInstruction, TransferOffset,
        decode_single_data_transfer,
    };

    use crate::cpu::arm::{ArmCondition, ShiftType};

    #[test]
    fn decodes_ldr_immediate_offset() {
        // LDR R0, [R1, #4]
        assert_eq!(
            decode_single_data_transfer(0xE591_0004),
            Ok(SingleDataTransferInstruction {
                condition: ArmCondition::Always,
                pre_index: true,
                add_offset: true,
                byte: false,
                write_back: false,
                force_user_mode: false,
                load: true,
                rn: 1,
                rd: 0,
                offset: TransferOffset::Immediate(4),
            })
        );
    }

    #[test]
    fn decodes_str_immediate_offset() {
        // STR R0, [R1, #4]
        let decoded = decode_single_data_transfer(0xE581_0004).unwrap();

        assert!(!decoded.load);
        assert!(!decoded.byte);
        assert_eq!(decoded.rn, 1);
        assert_eq!(decoded.rd, 0);
    }

    #[test]
    fn decodes_ldrb() {
        // LDRB R0, [R1]
        let decoded = decode_single_data_transfer(0xE5D1_0000).unwrap();

        assert!(decoded.load);
        assert!(decoded.byte);
    }

    #[test]
    fn decodes_strb() {
        // STRB R0, [R1]
        let decoded = decode_single_data_transfer(0xE5C1_0000).unwrap();

        assert!(!decoded.load);
        assert!(decoded.byte);
    }

    #[test]
    fn decodes_negative_offset() {
        // LDR R0, [R1, #-4]
        let decoded = decode_single_data_transfer(0xE511_0004).unwrap();

        assert!(!decoded.add_offset);
        assert_eq!(decoded.offset, TransferOffset::Immediate(4));
    }

    #[test]
    fn decodes_pre_index_writeback() {
        // LDR R0, [R1, #4]!
        let decoded = decode_single_data_transfer(0xE5B1_0004).unwrap();

        assert!(decoded.pre_index);
        assert!(decoded.write_back);
        assert!(!decoded.force_user_mode);
    }

    #[test]
    fn decodes_post_index() {
        // LDR R0, [R1], #4
        let decoded = decode_single_data_transfer(0xE491_0004).unwrap();

        assert!(!decoded.pre_index);
        assert!(decoded.write_back);
        assert!(!decoded.force_user_mode);
    }

    #[test]
    fn decodes_post_index_user_mode_variant() {
        // LDRT R0, [R1], #4
        let decoded = decode_single_data_transfer(0xE4B1_0004).unwrap();

        assert!(!decoded.pre_index);
        assert!(decoded.write_back);
        assert!(decoded.force_user_mode);
    }

    #[test]
    fn decodes_shifted_register_offset() {
        // LDR R0, [R1, R2, LSL #2]
        let decoded = decode_single_data_transfer(0xE791_0102).unwrap();

        assert_eq!(
            decoded.offset,
            TransferOffset::ShiftedRegister {
                rm: 2,
                shift_type: ShiftType::LogicalLeft,
                shift_amount: 2,
            }
        );
    }

    #[test]
    fn rejects_register_controlled_shift() {
        /*
         * Bit 4 set is invalid for single-data-transfer
         * register offset.
         */
        assert_eq!(
            decode_single_data_transfer(0xE791_0012),
            Err(SingleDataTransferDecodeError::InvalidRegisterOffset)
        );
    }

    #[test]
    fn rejects_pc_as_offset_register() {
        assert_eq!(
            decode_single_data_transfer(0xE791_000F),
            Err(SingleDataTransferDecodeError::ProgramCounterAsOffsetRegister)
        );
    }

    #[test]
    fn rejects_non_transfer_instruction() {
        assert_eq!(
            decode_single_data_transfer(0xE1A0_0000),
            Err(SingleDataTransferDecodeError::InvalidEncoding)
        );
    }
}
