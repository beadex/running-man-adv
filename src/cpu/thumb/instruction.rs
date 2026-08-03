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
pub enum ThumbAluOperation {
    And,
    ExclusiveOr,
    LogicalShiftLeft,
    LogicalShiftRight,
    ArithmeticShiftRight,
    AddWithCarry,
    SubtractWithCarry,
    RotateRight,
    Test,
    Negate,
    Compare,
    CompareNegative,
    Or,
    Multiply,
    BitClear,
    MoveNot,
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
pub enum ThumbRegisterOffsetTransferKind {
    StoreWord,
    StoreHalfword,
    StoreByte,
    LoadSignedByte,
    LoadWord,
    LoadHalfword,
    LoadByte,
    LoadSignedHalfword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbImmediateTransferKind {
    StoreWord,
    LoadWord,
    StoreByte,
    LoadByte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbHalfwordTransferKind {
    Store,
    Load,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbSpRelativeTransferKind {
    Store,
    Load,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbLoadAddressBase {
    ProgramCounter,
    StackPointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbStackPointerOperation {
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbLongBranchHalf {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbInstruction {
    MoveShiftedRegister {
        operation: ThumbShiftOperation,
        offset: u8,
        source: u8,
        destination: u8,
    },

    AddSubtract {
        operation: ThumbAddSubtractOperation,
        operand: ThumbAddSubtractOperand,
        source: u8,
        destination: u8,
    },

    Immediate {
        operation: ThumbImmediateOperation,
        destination: u8,
        immediate: u8,
    },

    Alu {
        operation: ThumbAluOperation,
        source: u8,
        destination: u8,
    },

    HighRegister {
        operation: ThumbHighRegisterOperation,
        source: u8,
        destination: u8,
    },

    LiteralLoad {
        destination: u8,
        offset: u16,
    },

    RegisterOffsetTransfer {
        kind: ThumbRegisterOffsetTransferKind,
        offset_register: u8,
        base_register: u8,
        destination: u8,
    },

    ImmediateOffsetTransfer {
        kind: ThumbImmediateTransferKind,
        offset: u8,
        base_register: u8,
        destination: u8,
    },

    HalfwordImmediateTransfer {
        kind: ThumbHalfwordTransferKind,
        offset: u8,
        base_register: u8,
        destination: u8,
    },

    SpRelativeTransfer {
        kind: ThumbSpRelativeTransferKind,
        destination: u8,
        offset: u16,
    },

    LoadAddress {
        base: ThumbLoadAddressBase,
        destination: u8,
        offset: u16,
    },

    AdjustStackPointer {
        operation: ThumbStackPointerOperation,
        offset: u16,
    },

    Push {
        registers: u8,
        include_link_register: bool,
    },

    Pop {
        registers: u8,
        include_program_counter: bool,
    },

    MultipleTransfer {
        load: bool,
        base_register: u8,
        registers: u8,
    },

    ConditionalBranch {
        condition: ThumbCondition,
        offset: i16,
    },

    SoftwareInterrupt {
        comment: u8,
    },

    UnconditionalBranch {
        offset: i32,
    },

    LongBranchWithLink {
        half: ThumbLongBranchHalf,
        offset: i32,
    },
}
