use crate::cpu::Cpsr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstructionKind {
    /// BX Rn
    BranchExchange,

    /// MUL / MLA
    Multiply,

    /// UMULL / UMLAL / SMULL / SMLAL
    MultiplyLong,

    /// SWP / SWPB
    SingleDataSwap,

    /// STRH / LDRH / LDRSB / LDRSH
    HalfwordDataTransfer,

    /// ALU and logical operations.
    ///
    /// Examples:
    /// AND, EOR, SUB, ADD, ADC, CMP, ORR, MOV, BIC, MVN.
    ///
    /// PSR transfer encodings are temporarily grouped here because
    /// they share this major encoding space. We will separate them
    /// when implementing the detailed decoder.
    DataProcessing,

    /// LDR / STR, including byte variants.
    SingleDataTransfer,

    /// LDM / STM.
    BlockDataTransfer,

    /// B / BL.
    Branch,

    /// LDC / STC.
    CoprocessorDataTransfer,

    /// CDP.
    CoprocessorDataOperation,

    /// MCR / MRC.
    CoprocessorRegisterTransfer,

    /// SWI.
    SoftwareInterrupt,

    /// An encoding that is undefined for ARMv4T.
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArmCondition {
    Equal = 0x0,
    NotEqual = 0x1,
    CarrySet = 0x2,
    CarryClear = 0x3,
    Minus = 0x4,
    Plus = 0x5,
    Overflow = 0x6,
    NoOverflow = 0x7,
    UnsignedHigher = 0x8,
    UnsignedLowerOrSame = 0x9,
    SignedGreaterOrEqual = 0xA,
    SignedLessThan = 0xB,
    SignedGreaterThan = 0xC,
    SignedLessOrEqual = 0xD,
    Always = 0xE,
    Never = 0xF,
}

impl ArmCondition {
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0xF {
            0x0 => Self::Equal,
            0x1 => Self::NotEqual,
            0x2 => Self::CarrySet,
            0x3 => Self::CarryClear,
            0x4 => Self::Minus,
            0x5 => Self::Plus,
            0x6 => Self::Overflow,
            0x7 => Self::NoOverflow,
            0x8 => Self::UnsignedHigher,
            0x9 => Self::UnsignedLowerOrSame,
            0xA => Self::SignedGreaterOrEqual,
            0xB => Self::SignedLessThan,
            0xC => Self::SignedGreaterThan,
            0xD => Self::SignedLessOrEqual,
            0xE => Self::Always,
            0xF => Self::Never,
            _ => unreachable!(),
        }
    }

    pub const fn evaluate(self, cpsr: Cpsr) -> bool {
        let n = cpsr.negative();
        let z = cpsr.zero();
        let c = cpsr.carry();
        let v = cpsr.overflow();

        match self {
            Self::Equal => z,
            Self::NotEqual => !z,

            Self::CarrySet => c,
            Self::CarryClear => !c,

            Self::Minus => n,
            Self::Plus => !n,

            Self::Overflow => v,
            Self::NoOverflow => !v,

            Self::UnsignedHigher => c && !z,
            Self::UnsignedLowerOrSame => !c || z,

            Self::SignedGreaterOrEqual => n == v,
            Self::SignedLessThan => n != v,

            Self::SignedGreaterThan => !z && n == v,
            Self::SignedLessOrEqual => z || n != v,

            Self::Always => true,

            /*
             * ARMv4T defines condition 0b1111 as Never.
             *
             * Later ARM architectures reuse parts of this encoding
             * space for unconditional instructions, but those do not
             * apply to the ARM7TDMI.
             */
            Self::Never => false,
        }
    }
}

pub const fn condition(instruction: u32) -> ArmCondition {
    ArmCondition::from_bits((instruction >> 28) as u8)
}

pub const fn condition_passed(instruction: u32, cpsr: Cpsr) -> bool {
    condition(instruction).evaluate(cpsr)
}

pub const fn classify(instruction: u32) -> ArmInstructionKind {
    /*
     * The condition field at bits 31..28 does not identify the
     * instruction class, so all masks below deliberately ignore it.
     *
     * Specific encodings must be tested before their broader
     * instruction families because several ARM encodings overlap.
     */

    if is_branch_exchange(instruction) {
        ArmInstructionKind::BranchExchange
    } else if is_multiply(instruction) {
        ArmInstructionKind::Multiply
    } else if is_multiply_long(instruction) {
        ArmInstructionKind::MultiplyLong
    } else if is_single_data_swap(instruction) {
        ArmInstructionKind::SingleDataSwap
    } else if is_halfword_data_transfer(instruction) {
        ArmInstructionKind::HalfwordDataTransfer
    } else {
        classify_major_group(instruction)
    }
}

const fn classify_major_group(instruction: u32) -> ArmInstructionKind {
    let bits_27_25 = (instruction >> 25) & 0b111;

    match bits_27_25 {
        /*
         * 000 and 001 normally contain data-processing instructions.
         *
         * Special 000 encodings such as multiply, swap, BX and
         * halfword transfer have already been handled above.
         */
        0b000 | 0b001 => ArmInstructionKind::DataProcessing,

        /*
         * 010: single data transfer with immediate offset.
         *
         * 011: single data transfer with register offset, except that
         * bit 4 set is undefined in the ARMv4T encoding space.
         */
        0b010 => ArmInstructionKind::SingleDataTransfer,

        0b011 => {
            if instruction & bit(4) == 0 {
                ArmInstructionKind::SingleDataTransfer
            } else {
                ArmInstructionKind::Undefined
            }
        }

        0b100 => ArmInstructionKind::BlockDataTransfer,

        0b101 => ArmInstructionKind::Branch,

        0b110 => ArmInstructionKind::CoprocessorDataTransfer,

        /*
         * For major group 111:
         *
         * bit 24 = 1: SWI
         * bit 24 = 0: coprocessor instruction
         *
         * For the coprocessor form:
         *
         * bit 4 = 0: CDP
         * bit 4 = 1: MCR/MRC
         */
        0b111 => {
            if instruction & bit(24) != 0 {
                ArmInstructionKind::SoftwareInterrupt
            } else if instruction & bit(4) == 0 {
                ArmInstructionKind::CoprocessorDataOperation
            } else {
                ArmInstructionKind::CoprocessorRegisterTransfer
            }
        }

        _ => unreachable!(),
    }
}

const fn is_branch_exchange(instruction: u32) -> bool {
    /*
     * BX:
     *
     * cond 0001 0010 1111 1111 1111 0001 Rn
     */
    instruction & 0x0FFF_FFF0 == 0x012F_FF10
}

const fn is_multiply(instruction: u32) -> bool {
    /*
     * MUL / MLA:
     *
     * cond 0000 00AS dddd nnnn ssss 1001 mmmm
     */
    instruction & 0x0FC0_00F0 == 0x0000_0090
}

const fn is_multiply_long(instruction: u32) -> bool {
    /*
     * UMULL / UMLAL / SMULL / SMLAL:
     *
     * cond 0000 1UAS hhhh llll ssss 1001 mmmm
     */
    instruction & 0x0F80_00F0 == 0x0080_0090
}

const fn is_single_data_swap(instruction: u32) -> bool {
    /*
     * SWP / SWPB:
     *
     * cond 0001 0B00 nnnn dddd 0000 1001 mmmm
     */
    instruction & 0x0FB0_0FF0 == 0x0100_0090
}

const fn is_halfword_data_transfer(instruction: u32) -> bool {
    /*
     * Halfword and signed data transfer:
     *
     * cond 000P UIWL nnnn dddd .... 1SH1 ....
     *
     * bits 7 and 4 must be set.
     * bits 6..5 select:
     *
     * 01: unsigned halfword
     * 10: signed byte
     * 11: signed halfword
     *
     * 00 belongs to other encoding families and must not be
     * classified as a halfword transfer.
     */
    let bits_27_25_are_zero = instruction & 0x0E00_0000 == 0;
    let bits_7_and_4_are_set = instruction & 0x0000_0090 == 0x0000_0090;
    let transfer_type = (instruction >> 5) & 0b11;

    bits_27_25_are_zero && bits_7_and_4_are_set && transfer_type != 0
}

const fn bit(index: u32) -> u32 {
    1u32 << index
}

#[cfg(test)]
mod tests {
    use super::{ArmCondition, ArmInstructionKind, classify, condition, condition_passed};
    use crate::cpu::Cpsr;

    fn cpsr(n: bool, z: bool, c: bool, v: bool) -> Cpsr {
        let mut cpsr = Cpsr::new();
        cpsr.set_nzcv(n, z, c, v);
        cpsr
    }

    #[test]
    fn extracts_condition_field() {
        assert_eq!(condition(0x0A00_0000), ArmCondition::Equal);

        assert_eq!(condition(0x1A00_0000), ArmCondition::NotEqual);

        assert_eq!(condition(0xEA00_0000), ArmCondition::Always);
    }

    #[test]
    fn classifies_branch_exchange() {
        // BX LR
        assert_eq!(classify(0xE12F_FF1E), ArmInstructionKind::BranchExchange);

        // BX R0
        assert_eq!(classify(0xE12F_FF10), ArmInstructionKind::BranchExchange);
    }

    #[test]
    fn classifies_multiply() {
        // MUL R0, R1, R2
        assert_eq!(classify(0xE000_0291), ArmInstructionKind::Multiply);

        // MLA R0, R1, R2, R3
        assert_eq!(classify(0xE020_3291), ArmInstructionKind::Multiply);
    }

    #[test]
    fn classifies_multiply_long() {
        // UMULL R0, R1, R2, R3
        assert_eq!(classify(0xE081_0392), ArmInstructionKind::MultiplyLong);

        // SMULL R0, R1, R2, R3
        assert_eq!(classify(0xE0C1_0392), ArmInstructionKind::MultiplyLong);
    }

    #[test]
    fn classifies_single_data_swap() {
        // SWP R0, R1, [R2]
        assert_eq!(classify(0xE102_0091), ArmInstructionKind::SingleDataSwap);

        // SWPB R0, R1, [R2]
        assert_eq!(classify(0xE142_0091), ArmInstructionKind::SingleDataSwap);
    }

    #[test]
    fn classifies_halfword_data_transfer() {
        // STRH R1, [R0]
        assert_eq!(
            classify(0xE1C0_10B0),
            ArmInstructionKind::HalfwordDataTransfer
        );

        // LDRH R1, [R0]
        assert_eq!(
            classify(0xE1D0_10B0),
            ArmInstructionKind::HalfwordDataTransfer
        );

        // LDRSB R1, [R0]
        assert_eq!(
            classify(0xE1D0_10D0),
            ArmInstructionKind::HalfwordDataTransfer
        );

        // LDRSH R1, [R0]
        assert_eq!(
            classify(0xE1D0_10F0),
            ArmInstructionKind::HalfwordDataTransfer
        );
    }

    #[test]
    fn classifies_data_processing() {
        // MOV R0, R0
        assert_eq!(classify(0xE1A0_0000), ArmInstructionKind::DataProcessing);

        // ADD R0, R1, R2
        assert_eq!(classify(0xE081_0002), ArmInstructionKind::DataProcessing);

        // CMP R0, #1
        assert_eq!(classify(0xE350_0001), ArmInstructionKind::DataProcessing);
    }

    #[test]
    fn classifies_single_data_transfer() {
        // LDR R0, [R1]
        assert_eq!(
            classify(0xE591_0000),
            ArmInstructionKind::SingleDataTransfer
        );

        // STR R0, [R1]
        assert_eq!(
            classify(0xE581_0000),
            ArmInstructionKind::SingleDataTransfer
        );

        // LDR R0, [R1, R2]
        assert_eq!(
            classify(0xE791_0002),
            ArmInstructionKind::SingleDataTransfer
        );
    }

    #[test]
    fn register_shifted_register_transfer_encoding_is_undefined() {
        /*
         * Major group 011 with bit 4 set is not a valid
         * single-data-transfer encoding in ARMv4T.
         */
        assert_eq!(classify(0xE791_0012), ArmInstructionKind::Undefined);
    }

    #[test]
    fn classifies_block_data_transfer() {
        // STMIA R0!, {R1, R2}
        assert_eq!(classify(0xE8A0_0006), ArmInstructionKind::BlockDataTransfer);

        // LDMIA R0!, {R1, R2}
        assert_eq!(classify(0xE8B0_0006), ArmInstructionKind::BlockDataTransfer);
    }

    #[test]
    fn classifies_branch() {
        // B
        assert_eq!(classify(0xEA00_0000), ArmInstructionKind::Branch);

        // BL
        assert_eq!(classify(0xEB00_0000), ArmInstructionKind::Branch);
    }

    #[test]
    fn classifies_coprocessor_data_transfer() {
        // Representative LDC/STC encoding.
        assert_eq!(
            classify(0xEC00_0000),
            ArmInstructionKind::CoprocessorDataTransfer
        );
    }

    #[test]
    fn classifies_coprocessor_data_operation() {
        // Representative CDP encoding.
        assert_eq!(
            classify(0xEE00_0000),
            ArmInstructionKind::CoprocessorDataOperation
        );
    }

    #[test]
    fn classifies_coprocessor_register_transfer() {
        // Representative MCR/MRC encoding.
        assert_eq!(
            classify(0xEE00_0010),
            ArmInstructionKind::CoprocessorRegisterTransfer
        );
    }

    #[test]
    fn classifies_software_interrupt() {
        // SWI 0
        assert_eq!(classify(0xEF00_0000), ArmInstructionKind::SoftwareInterrupt);

        // SWI with a non-zero comment field.
        assert_eq!(classify(0xEF12_3456), ArmInstructionKind::SoftwareInterrupt);
    }

    #[test]
    fn instruction_class_does_not_depend_on_condition() {
        let always_add = 0xE081_0002;
        let equal_add = 0x0081_0002;
        let not_equal_add = 0x1081_0002;

        assert_eq!(classify(always_add), ArmInstructionKind::DataProcessing);

        assert_eq!(classify(equal_add), ArmInstructionKind::DataProcessing);

        assert_eq!(classify(not_equal_add), ArmInstructionKind::DataProcessing);
    }

    #[test]
    fn evaluates_equal_condition() {
        assert!(ArmCondition::Equal.evaluate(cpsr(false, true, false, false)));

        assert!(!ArmCondition::Equal.evaluate(cpsr(false, false, false, false)));
    }

    #[test]
    fn evaluates_not_equal_condition() {
        assert!(ArmCondition::NotEqual.evaluate(cpsr(false, false, false, false)));

        assert!(!ArmCondition::NotEqual.evaluate(cpsr(false, true, false, false)));
    }

    #[test]
    fn evaluates_carry_conditions() {
        assert!(ArmCondition::CarrySet.evaluate(cpsr(false, false, true, false)));

        assert!(!ArmCondition::CarrySet.evaluate(cpsr(false, false, false, false)));

        assert!(ArmCondition::CarryClear.evaluate(cpsr(false, false, false, false)));

        assert!(!ArmCondition::CarryClear.evaluate(cpsr(false, false, true, false)));
    }

    #[test]
    fn evaluates_sign_conditions() {
        assert!(ArmCondition::Minus.evaluate(cpsr(true, false, false, false)));

        assert!(!ArmCondition::Minus.evaluate(cpsr(false, false, false, false)));

        assert!(ArmCondition::Plus.evaluate(cpsr(false, false, false, false)));

        assert!(!ArmCondition::Plus.evaluate(cpsr(true, false, false, false)));
    }

    #[test]
    fn evaluates_overflow_conditions() {
        assert!(ArmCondition::Overflow.evaluate(cpsr(false, false, false, true)));

        assert!(!ArmCondition::Overflow.evaluate(cpsr(false, false, false, false)));

        assert!(ArmCondition::NoOverflow.evaluate(cpsr(false, false, false, false)));

        assert!(!ArmCondition::NoOverflow.evaluate(cpsr(false, false, false, true)));
    }

    #[test]
    fn evaluates_unsigned_higher_condition() {
        assert!(ArmCondition::UnsignedHigher.evaluate(cpsr(false, false, true, false)));

        // Carry clear means unsigned lower.
        assert!(!ArmCondition::UnsignedHigher.evaluate(cpsr(false, false, false, false)));

        // Zero means equal, not higher.
        assert!(!ArmCondition::UnsignedHigher.evaluate(cpsr(false, true, true, false)));
    }

    #[test]
    fn evaluates_unsigned_lower_or_same_condition() {
        // Lower: carry clear.
        assert!(ArmCondition::UnsignedLowerOrSame.evaluate(cpsr(false, false, false, false)));

        // Same: zero set.
        assert!(ArmCondition::UnsignedLowerOrSame.evaluate(cpsr(false, true, true, false)));

        // Carry set and non-zero means higher.
        assert!(!ArmCondition::UnsignedLowerOrSame.evaluate(cpsr(false, false, true, false)));
    }

    #[test]
    fn evaluates_signed_greater_or_equal_condition() {
        // Positive result without overflow.
        assert!(ArmCondition::SignedGreaterOrEqual.evaluate(cpsr(false, false, false, false)));

        // Negative result with overflow: N == V.
        assert!(ArmCondition::SignedGreaterOrEqual.evaluate(cpsr(true, false, false, true)));

        assert!(!ArmCondition::SignedGreaterOrEqual.evaluate(cpsr(true, false, false, false)));
    }

    #[test]
    fn evaluates_signed_less_than_condition() {
        assert!(ArmCondition::SignedLessThan.evaluate(cpsr(true, false, false, false)));

        assert!(ArmCondition::SignedLessThan.evaluate(cpsr(false, false, false, true)));

        assert!(!ArmCondition::SignedLessThan.evaluate(cpsr(false, false, false, false)));
    }

    #[test]
    fn evaluates_signed_greater_than_condition() {
        assert!(ArmCondition::SignedGreaterThan.evaluate(cpsr(false, false, false, false)));

        // Equal cannot be greater than.
        assert!(!ArmCondition::SignedGreaterThan.evaluate(cpsr(false, true, false, false)));

        // N != V means signed less than.
        assert!(!ArmCondition::SignedGreaterThan.evaluate(cpsr(true, false, false, false)));
    }

    #[test]
    fn evaluates_signed_less_or_equal_condition() {
        // Equal.
        assert!(ArmCondition::SignedLessOrEqual.evaluate(cpsr(false, true, false, false)));

        // Signed less than because N != V.
        assert!(ArmCondition::SignedLessOrEqual.evaluate(cpsr(true, false, false, false)));

        assert!(!ArmCondition::SignedLessOrEqual.evaluate(cpsr(false, false, false, false)));
    }

    #[test]
    fn evaluates_always_and_never_conditions() {
        let flags = cpsr(true, true, true, true);

        assert!(ArmCondition::Always.evaluate(flags));
        assert!(!ArmCondition::Never.evaluate(flags));
    }

    #[test]
    fn evaluates_condition_from_instruction() {
        /*
         * ADDEQ R0, R1, R2
         *
         * cond = EQ
         */
        let instruction = 0x0081_0002;

        assert!(condition_passed(
            instruction,
            cpsr(false, true, false, false)
        ));

        assert!(!condition_passed(
            instruction,
            cpsr(false, false, false, false)
        ));
    }

    #[test]
    fn all_conditions_match_reference_expressions() {
        for bits in 0u32..16 {
            let n = bits & 0b1000 != 0;
            let z = bits & 0b0100 != 0;
            let c = bits & 0b0010 != 0;
            let v = bits & 0b0001 != 0;

            let flags = cpsr(n, z, c, v);

            let expected = [
                z,
                !z,
                c,
                !c,
                n,
                !n,
                v,
                !v,
                c && !z,
                !c || z,
                n == v,
                n != v,
                !z && n == v,
                z || n != v,
                true,
                false,
            ];

            for condition_bits in 0u8..16 {
                let condition = ArmCondition::from_bits(condition_bits);

                assert_eq!(
                    condition.evaluate(flags),
                    expected[condition_bits as usize],
                    "condition={condition:?}, \
                 N={n}, Z={z}, C={c}, V={v}"
                );
            }
        }
    }
}
