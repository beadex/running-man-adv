use super::ArmCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAddressingMode {
    IncrementAfter,
    IncrementBefore,
    DecrementAfter,
    DecrementBefore,
}

impl BlockAddressingMode {
    pub const fn from_bits(pre_index: bool, add: bool) -> Self {
        match (pre_index, add) {
            (false, true) => Self::IncrementAfter,
            (true, true) => Self::IncrementBefore,
            (false, false) => Self::DecrementAfter,
            (true, false) => Self::DecrementBefore,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterList {
    bits: u16,
}

impl RegisterList {
    pub const fn new(bits: u16) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn contains(self, register: u8) -> bool {
        register < 16 && self.bits & (1u16 << register) != 0
    }

    pub const fn count(self) -> u32 {
        self.bits.count_ones()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDataTransferInstruction {
    pub condition: ArmCondition,
    pub addressing_mode: BlockAddressingMode,

    /// S bit.
    ///
    /// This has user-register-bank and SPSR semantics and is
    /// temporarily rejected by the executor.
    pub psr_or_user_mode: bool,

    pub write_back: bool,
    pub load: bool,
    pub rn: u8,
    pub registers: RegisterList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDataTransferDecodeError {
    InvalidEncoding,
    EmptyRegisterList,
}

pub fn decode_block_data_transfer(
    instruction: u32,
) -> Result<BlockDataTransferInstruction, BlockDataTransferDecodeError> {
    /*
     * Block Data Transfer requires bits 27..25 = 100.
     */
    if instruction & 0x0E00_0000 != 0x0800_0000 {
        return Err(BlockDataTransferDecodeError::InvalidEncoding);
    }

    let pre_index = instruction & (1 << 24) != 0;
    let add = instruction & (1 << 23) != 0;
    let psr_or_user_mode = instruction & (1 << 22) != 0;
    let write_back = instruction & (1 << 21) != 0;
    let load = instruction & (1 << 20) != 0;

    let rn = ((instruction >> 16) & 0x0F) as u8;
    let registers = RegisterList::new(instruction as u16);

    /*
     * ARM7TDMI has special behavior for an empty register list.
     *
     * We reject it until that architecture-specific edge case is
     * implemented intentionally.
     */
    if registers.is_empty() {
        return Err(BlockDataTransferDecodeError::EmptyRegisterList);
    }

    Ok(BlockDataTransferInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        addressing_mode: BlockAddressingMode::from_bits(pre_index, add),
        psr_or_user_mode,
        write_back,
        load,
        rn,
        registers,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BlockAddressingMode, BlockDataTransferDecodeError, RegisterList, decode_block_data_transfer,
    };

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn register_list_reports_membership_and_count() {
        let list = RegisterList::new((1 << 0) | (1 << 3) | (1 << 14));

        assert!(list.contains(0));
        assert!(list.contains(3));
        assert!(list.contains(14));
        assert!(!list.contains(1));
        assert_eq!(list.count(), 3);
    }

    #[test]
    fn decodes_stmia() {
        /*
         * STMIA R0!, {R1, R2}
         */
        let decoded = decode_block_data_transfer(0xE8A0_0006).unwrap();

        assert_eq!(decoded.condition, ArmCondition::Always);
        assert_eq!(decoded.addressing_mode, BlockAddressingMode::IncrementAfter);

        assert!(!decoded.load);
        assert!(decoded.write_back);
        assert_eq!(decoded.rn, 0);
        assert!(decoded.registers.contains(1));
        assert!(decoded.registers.contains(2));
        assert_eq!(decoded.registers.count(), 2);
    }

    #[test]
    fn decodes_ldmia() {
        // LDMIA R0!, {R1, R2}
        let decoded = decode_block_data_transfer(0xE8B0_0006).unwrap();

        assert!(decoded.load);
        assert!(decoded.write_back);

        assert_eq!(decoded.addressing_mode, BlockAddressingMode::IncrementAfter);
    }

    #[test]
    fn decodes_stmdb_stack_push() {
        /*
         * STMDB SP!, {R4-R7, LR}
         *
         * Equivalent stack alias:
         * STMFD SP!, {R4-R7, LR}
         */
        let decoded = decode_block_data_transfer(0xE92D_40F0).unwrap();

        assert!(!decoded.load);
        assert!(decoded.write_back);
        assert_eq!(decoded.rn, 13);

        assert_eq!(
            decoded.addressing_mode,
            BlockAddressingMode::DecrementBefore
        );

        for register in 4..=7 {
            assert!(decoded.registers.contains(register));
        }

        assert!(decoded.registers.contains(14));
        assert_eq!(decoded.registers.count(), 5);
    }

    #[test]
    fn decodes_ldmia_stack_pop() {
        /*
         * LDMIA SP!, {R4-R7, PC}
         *
         * Equivalent stack alias:
         * LDMFD SP!, {R4-R7, PC}
         */
        let decoded = decode_block_data_transfer(0xE8BD_80F0).unwrap();

        assert!(decoded.load);
        assert!(decoded.write_back);
        assert_eq!(decoded.rn, 13);

        assert_eq!(decoded.addressing_mode, BlockAddressingMode::IncrementAfter);

        assert!(decoded.registers.contains(15));
    }

    #[test]
    fn decodes_increment_before() {
        let decoded = decode_block_data_transfer(0xE9A0_0006).unwrap();

        assert_eq!(
            decoded.addressing_mode,
            BlockAddressingMode::IncrementBefore
        );
    }

    #[test]
    fn decodes_decrement_after() {
        let decoded = decode_block_data_transfer(0xE820_0006).unwrap();

        assert_eq!(decoded.addressing_mode, BlockAddressingMode::DecrementAfter);
    }

    #[test]
    fn preserves_s_bit() {
        let decoded = decode_block_data_transfer(0xE8E0_0006).unwrap();

        assert!(decoded.psr_or_user_mode);
    }

    #[test]
    fn rejects_empty_register_list() {
        assert_eq!(
            decode_block_data_transfer(0xE8A0_0000),
            Err(BlockDataTransferDecodeError::EmptyRegisterList)
        );
    }

    #[test]
    fn rejects_non_block_transfer() {
        assert_eq!(
            decode_block_data_transfer(0xE1A0_0000),
            Err(BlockDataTransferDecodeError::InvalidEncoding)
        );
    }
}
