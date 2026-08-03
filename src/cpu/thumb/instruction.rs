#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbShiftOperation {
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbAddSubtractOperation {
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbAddSubtractOperand {
    Register(u8),
    Immediate(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbImmediateOperation {
    Move,
    Compare,
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbHighRegisterOperation {
    Add,
    Compare,
    Move,
    BranchExchange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbCondition {
    Equal,
    NotEqual,

    CarrySet,
    CarryClear,

    Minus,
    Plus,

    Overflow,
    NoOverflow,

    UnsignedHigher,
    UnsignedLowerOrSame,

    SignedGreaterOrEqual,
    SignedLessThan,

    SignedGreaterThan,
    SignedLessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbInstruction {
    /*
     * Format 1:
     *
     * LSL Rd, Rs, #imm5
     * LSR Rd, Rs, #imm5
     * ASR Rd, Rs, #imm5
     */
    MoveShiftedRegister {
        operation: ThumbShiftOperation,
        offset: u8,
        source: u8,
        destination: u8,
    },

    /*
     * Format 2:
     *
     * ADD/SUB Rd, Rs, Rn
     * ADD/SUB Rd, Rs, #imm3
     */
    AddSubtract {
        operation: ThumbAddSubtractOperation,

        operand: ThumbAddSubtractOperand,

        source: u8,
        destination: u8,
    },

    /*
     * Format 3:
     *
     * MOV Rd, #imm8
     * CMP Rd, #imm8
     * ADD Rd, #imm8
     * SUB Rd, #imm8
     */
    Immediate {
        operation: ThumbImmediateOperation,

        destination: u8,
        immediate: u8,
    },

    /*
     * Format 5:
     *
     * ADD Hd, Hs
     * CMP Hd, Hs
     * MOV Hd, Hs
     * BX Hs
     */
    HighRegister {
        operation: ThumbHighRegisterOperation,

        source: u8,
        destination: u8,
    },

    /*
     * Format 16:
     *
     * B<condition> label
     */
    ConditionalBranch {
        condition: ThumbCondition,

        /*
         * Signed byte offset after multiplying by two.
         */
        offset: i16,
    },

    /*
     * Format 17:
     *
     * SWI #imm8
     */
    SoftwareInterrupt {
        comment: u8,
    },

    /*
     * Format 18:
     *
     * B label
     */
    UnconditionalBranch {
        /*
         * Signed 11-bit offset after multiplying by two.
         */
        offset: i32,
    },
}
