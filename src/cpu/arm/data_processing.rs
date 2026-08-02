use crate::cpu::Registers;

use super::{ArmCondition, ShiftResult, expand_rotated_immediate, shift_immediate, shift_register};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProcessingOpcode {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}

impl DataProcessingOpcode {
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x0F {
            0x0 => Self::And,
            0x1 => Self::Eor,
            0x2 => Self::Sub,
            0x3 => Self::Rsb,
            0x4 => Self::Add,
            0x5 => Self::Adc,
            0x6 => Self::Sbc,
            0x7 => Self::Rsc,
            0x8 => Self::Tst,
            0x9 => Self::Teq,
            0xA => Self::Cmp,
            0xB => Self::Cmn,
            0xC => Self::Orr,
            0xD => Self::Mov,
            0xE => Self::Bic,
            0xF => Self::Mvn,
            _ => unreachable!(),
        }
    }

    pub const fn is_test(self) -> bool {
        matches!(self, Self::Tst | Self::Teq | Self::Cmp | Self::Cmn)
    }

    pub const fn writes_result(self) -> bool {
        !self.is_test()
    }

    pub const fn uses_rn(self) -> bool {
        !matches!(self, Self::Mov | Self::Mvn)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftType {
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
    RotateRight,
}

impl ShiftType {
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::LogicalLeft,
            0b01 => Self::LogicalRight,
            0b10 => Self::ArithmeticRight,
            0b11 => Self::RotateRight,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftAmount {
    /// Shift amount encoded directly in bits 11..7.
    Immediate(u8),

    /// Shift amount is read from the bottom byte of register Rs.
    Register(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterShift {
    pub rm: u8,
    pub shift_type: ShiftType,
    pub amount: ShiftAmount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand2 {
    Immediate {
        /// Unrotated 8-bit immediate value.
        value: u8,

        /// Encoded rotation field from bits 11..8.
        ///
        /// The actual rotation in bits is `rotate * 2`.
        rotate: u8,
    },

    Register(RegisterShift),
}

impl Operand2 {
    /// Returns the expanded value for an immediate Operand2.
    ///
    /// The carry result is intentionally not calculated here because
    /// carry handling belongs to the future barrel-shifter semantics.
    pub const fn expanded_immediate(self) -> Option<u32> {
        match self {
            Self::Immediate { value, rotate } => {
                Some((value as u32).rotate_right((rotate as u32) * 2))
            }

            Self::Register(_) => None,
        }
    }

    /*
     * Temporary implementation.
     *
     * Reading R15 currently returns the raw stored PC. Once pipeline-visible
     * PC semantics are implemented, operand register reads must go through
     * a CPU-aware register-reading API.
     */
    pub fn evaluate(self, registers: &Registers, old_carry: bool) -> ShiftResult {
        match self {
            Self::Immediate { value, rotate } => expand_rotated_immediate(value, rotate, old_carry),

            Self::Register(shift) => {
                let value = registers.read(shift.rm as usize);

                match shift.amount {
                    ShiftAmount::Immediate(amount) => {
                        shift_immediate(value, shift.shift_type, amount, old_carry)
                    }

                    ShiftAmount::Register(rs) => {
                        let rs_value = registers.read(rs as usize);

                        shift_register(value, shift.shift_type, rs_value, old_carry)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataProcessingInstruction {
    pub condition: ArmCondition,
    pub opcode: DataProcessingOpcode,
    pub set_flags: bool,
    pub rn: u8,
    pub rd: u8,
    pub operand2: Operand2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProcessingDecodeError {
    /// Bits 27..26 must be zero for the data-processing encoding space.
    InvalidMajor,

    /// This encoding belongs to MRS, MSR, or another special encoding
    /// that shares the data-processing major instruction group.
    Special,

    /// Register-controlled shifts require bit 7 to be clear.
    ///
    /// Encodings with bits 7 and 4 set belong to other ARM instruction
    /// families such as multiply and halfword transfer.
    InvalidRegisterShift,
}

pub fn decode_data_processing(
    instruction: u32,
) -> Result<DataProcessingInstruction, DataProcessingDecodeError> {
    /*
     * Data-processing instructions occupy major encoding:
     *
     * bits 27..26 = 00
     */
    if instruction & 0x0C00_0000 != 0 {
        return Err(DataProcessingDecodeError::InvalidMajor);
    }

    let immediate = instruction & bit(25) != 0;
    let opcode = DataProcessingOpcode::from_bits(((instruction >> 21) & 0x0F) as u8);

    let set_flags = instruction & bit(20) != 0;
    let rn = ((instruction >> 16) & 0x0F) as u8;
    let rd = ((instruction >> 12) & 0x0F) as u8;

    /*
     * Opcodes TST, TEQ, CMP and CMN normally require S=1 and do not
     * write Rd.
     *
     * When opcode is 8..11 and S=0, this encoding space contains PSR
     * transfer instructions such as MRS and MSR, rather than normal
     * data-processing instructions.
     */
    if opcode.is_test() && !set_flags {
        return Err(DataProcessingDecodeError::Special);
    }

    let operand2 = if immediate {
        decode_immediate_operand2(instruction)
    } else {
        decode_register_operand2(instruction)?
    };

    Ok(DataProcessingInstruction {
        condition: ArmCondition::from_bits((instruction >> 28) as u8),
        opcode,
        set_flags,
        rn,
        rd,
        operand2,
    })
}

fn decode_immediate_operand2(instruction: u32) -> Operand2 {
    let value = (instruction & 0xFF) as u8;
    let rotate = ((instruction >> 8) & 0x0F) as u8;

    Operand2::Immediate { value, rotate }
}

fn decode_register_operand2(instruction: u32) -> Result<Operand2, DataProcessingDecodeError> {
    let rm = (instruction & 0x0F) as u8;
    let shift_type = ShiftType::from_bits(((instruction >> 5) & 0b11) as u8);

    let shift_by_register = instruction & bit(4) != 0;

    let amount = if shift_by_register {
        /*
         * A data-processing register-controlled shift has:
         *
         * bit 4 = 1
         * bit 7 = 0
         *
         * If bit 7 is also set, the encoding overlaps multiply,
         * swap, or halfword/signed-transfer instruction families.
         */
        if instruction & bit(7) != 0 {
            return Err(DataProcessingDecodeError::InvalidRegisterShift);
        }

        let rs = ((instruction >> 8) & 0x0F) as u8;

        ShiftAmount::Register(rs)
    } else {
        let shift_imm = ((instruction >> 7) & 0x1F) as u8;

        ShiftAmount::Immediate(shift_imm)
    };

    Ok(Operand2::Register(RegisterShift {
        rm,
        shift_type,
        amount,
    }))
}

const fn bit(index: u32) -> u32 {
    1u32 << index
}

#[cfg(test)]
mod tests {
    use super::{
        DataProcessingDecodeError, DataProcessingOpcode, Operand2, RegisterShift, ShiftAmount,
        ShiftType, decode_data_processing,
    };

    use crate::cpu::arm::ArmCondition;

    #[test]
    fn decodes_mov_register_without_shift() {
        /*
         * MOV R0, R1
         *
         * Operand2 is encoded as R1, LSL #0.
         */
        let decoded = decode_data_processing(0xE1A0_0001).unwrap();

        assert_eq!(decoded.condition, ArmCondition::Always);
        assert_eq!(decoded.opcode, DataProcessingOpcode::Mov);
        assert!(!decoded.set_flags);
        assert_eq!(decoded.rn, 0);
        assert_eq!(decoded.rd, 0);

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::LogicalLeft,
                amount: ShiftAmount::Immediate(0),
            })
        );
    }

    #[test]
    fn decodes_add_register() {
        // ADD R0, R1, R2
        let decoded = decode_data_processing(0xE081_0002).unwrap();

        assert_eq!(decoded.opcode, DataProcessingOpcode::Add);
        assert!(!decoded.set_flags);
        assert_eq!(decoded.rn, 1);
        assert_eq!(decoded.rd, 0);

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 2,
                shift_type: ShiftType::LogicalLeft,
                amount: ShiftAmount::Immediate(0),
            })
        );
    }

    #[test]
    fn decodes_add_and_set_flags() {
        // ADDS R0, R1, R2
        let decoded = decode_data_processing(0xE091_0002).unwrap();

        assert_eq!(decoded.opcode, DataProcessingOpcode::Add);
        assert!(decoded.set_flags);
        assert_eq!(decoded.rn, 1);
        assert_eq!(decoded.rd, 0);
    }

    #[test]
    fn decodes_immediate_operand() {
        // MOV R0, #1
        let decoded = decode_data_processing(0xE3A0_0001).unwrap();

        assert_eq!(decoded.opcode, DataProcessingOpcode::Mov);

        assert_eq!(
            decoded.operand2,
            Operand2::Immediate {
                value: 1,
                rotate: 0,
            }
        );

        assert_eq!(decoded.operand2.expanded_immediate(), Some(1));
    }

    #[test]
    fn decodes_rotated_immediate_operand() {
        /*
         * MOV R0, #0x01000000
         *
         * imm8 = 1
         * rotate = 4
         * actual rotation = 8 bits
         */
        let decoded = decode_data_processing(0xE3A0_0401).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Immediate {
                value: 1,
                rotate: 4,
            }
        );

        assert_eq!(decoded.operand2.expanded_immediate(), Some(0x0100_0000));
    }

    #[test]
    fn decodes_immediate_shift() {
        // MOV R0, R1, LSL #3
        let decoded = decode_data_processing(0xE1A0_0181).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::LogicalLeft,
                amount: ShiftAmount::Immediate(3),
            })
        );
    }

    #[test]
    fn decodes_logical_right_shift() {
        // MOV R0, R1, LSR #4
        let decoded = decode_data_processing(0xE1A0_0221).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::LogicalRight,
                amount: ShiftAmount::Immediate(4),
            })
        );
    }

    #[test]
    fn decodes_arithmetic_right_shift() {
        // MOV R0, R1, ASR #5
        let decoded = decode_data_processing(0xE1A0_02C1).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::ArithmeticRight,
                amount: ShiftAmount::Immediate(5),
            })
        );
    }

    #[test]
    fn decodes_rotate_right_shift() {
        // MOV R0, R1, ROR #8
        let decoded = decode_data_processing(0xE1A0_0461).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::RotateRight,
                amount: ShiftAmount::Immediate(8),
            })
        );
    }

    #[test]
    fn decodes_register_controlled_shift() {
        // MOV R0, R1, LSL R2
        let decoded = decode_data_processing(0xE1A0_0211).unwrap();

        assert_eq!(
            decoded.operand2,
            Operand2::Register(RegisterShift {
                rm: 1,
                shift_type: ShiftType::LogicalLeft,
                amount: ShiftAmount::Register(2),
            })
        );
    }

    #[test]
    fn decodes_cmp() {
        // CMP R1, R2
        let decoded = decode_data_processing(0xE151_0002).unwrap();

        assert_eq!(decoded.opcode, DataProcessingOpcode::Cmp);
        assert!(decoded.set_flags);
        assert_eq!(decoded.rn, 1);

        /*
         * Rd is physically present in the instruction word, but CMP
         * does not write a destination register.
         */
        assert!(!decoded.opcode.writes_result());
    }

    #[test]
    fn decodes_conditional_instruction() {
        // ADDEQ R0, R1, R2
        let decoded = decode_data_processing(0x0081_0002).unwrap();

        assert_eq!(decoded.condition, ArmCondition::Equal);
        assert_eq!(decoded.opcode, DataProcessingOpcode::Add);
    }

    #[test]
    fn mov_does_not_use_rn() {
        assert!(!DataProcessingOpcode::Mov.uses_rn());
        assert!(!DataProcessingOpcode::Mvn.uses_rn());
        assert!(DataProcessingOpcode::Add.uses_rn());
    }

    #[test]
    fn rejects_non_data_processing_major_group() {
        // B instruction
        let result = decode_data_processing(0xEA00_0000);

        assert_eq!(result, Err(DataProcessingDecodeError::InvalidMajor));
    }

    #[test]
    fn rejects_test_opcode_when_s_is_clear() {
        /*
         * Opcode 1000 with S=0 occupies a special encoding region,
         * rather than representing a normal TST instruction.
         */
        let result = decode_data_processing(0xE100_0000);

        assert_eq!(result, Err(DataProcessingDecodeError::Special));
    }

    #[test]
    fn rejects_invalid_register_shift_overlap() {
        /*
         * Register operand with both bit 7 and bit 4 set cannot be a
         * valid data-processing register-controlled shift.
         */
        let result = decode_data_processing(0xE1A0_0091);

        assert_eq!(result, Err(DataProcessingDecodeError::InvalidRegisterShift));
    }

    #[test]
    fn decodes_all_opcodes() {
        let expected = [
            DataProcessingOpcode::And,
            DataProcessingOpcode::Eor,
            DataProcessingOpcode::Sub,
            DataProcessingOpcode::Rsb,
            DataProcessingOpcode::Add,
            DataProcessingOpcode::Adc,
            DataProcessingOpcode::Sbc,
            DataProcessingOpcode::Rsc,
            DataProcessingOpcode::Tst,
            DataProcessingOpcode::Teq,
            DataProcessingOpcode::Cmp,
            DataProcessingOpcode::Cmn,
            DataProcessingOpcode::Orr,
            DataProcessingOpcode::Mov,
            DataProcessingOpcode::Bic,
            DataProcessingOpcode::Mvn,
        ];

        for opcode_bits in 0u8..16 {
            assert_eq!(
                DataProcessingOpcode::from_bits(opcode_bits),
                expected[opcode_bits as usize]
            );
        }
    }
}
