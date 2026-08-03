mod alu;
mod barrel_shifter;
mod branch;
mod branch_exchange;
mod branch_exchange_executor;
mod branch_executor;
mod data_processing;
mod data_processing_executor;
mod decoder;
mod executor;
mod instruction;
mod instruction_decoder;

pub use self::alu::{AddResult, AluFlags, add_with_carry, arithmetic_shift_flags, logical_flags};

pub use self::barrel_shifter::{
    ShiftResult, expand_rotated_immediate, shift_immediate, shift_register,
};

pub use self::branch::{BranchDecodeError, BranchInstruction, decode_branch};

pub use self::branch_exchange::{
    BranchExchangeDecodeError, BranchExchangeInstruction, decode_branch_exchange,
};

pub use self::branch_executor::{BranchExecutionResult, execute_branch};

pub use self::branch_exchange_executor::{
    BranchExchangeExecutionError, BranchExchangeExecutionResult, execute_branch_exchange,
};

pub use self::data_processing::{
    DataProcessingDecodeError, DataProcessingInstruction, DataProcessingOpcode, Operand2,
    RegisterShift, ShiftAmount, ShiftType, decode_data_processing,
};

pub use self::data_processing_executor::{DataProcessingExecutionError, execute_data_processing};

pub use self::decoder::{ArmCondition, ArmInstructionKind, classify, condition, condition_passed};

pub use self::executor::{ArmExecutionError, execute_arm};

pub use self::instruction::ArmInstruction;

pub use self::instruction_decoder::{ArmDecodeError, decode_arm};
