mod alu;
mod barrel_shifter;
mod block_data_transfer;
mod block_data_transfer_executor;
mod branch;
mod branch_exchange;
mod branch_exchange_executor;
mod branch_executor;
mod data_processing;
mod data_processing_executor;
mod decoder;
mod executor;
mod halfword_data_transfer;
mod halfword_data_transfer_executor;
mod instruction;
mod instruction_decoder;
mod multiply;
mod multiply_executor;
mod multiply_long;
mod multiply_long_executor;
mod single_data_transfer;
mod single_data_transfer_executor;
mod status_register;
mod status_register_executor;

pub use self::alu::{AddResult, AluFlags, add_with_carry, arithmetic_shift_flags, logical_flags};

pub use self::barrel_shifter::{
    ShiftResult, expand_rotated_immediate, shift_immediate, shift_register,
};

pub use self::block_data_transfer::{
    BlockAddressingMode, BlockDataTransferDecodeError, BlockDataTransferInstruction, RegisterList,
    decode_block_data_transfer,
};

pub use self::block_data_transfer_executor::{
    BlockDataTransferExecutionError, BlockDataTransferExecutionResult, execute_block_data_transfer,
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

pub use self::halfword_data_transfer::{
    HalfwordDataTransferDecodeError, HalfwordDataTransferInstruction, HalfwordTransferKind,
    HalfwordTransferOffset, decode_halfword_data_transfer,
};

pub use self::halfword_data_transfer_executor::{
    HalfwordDataTransferExecutionError, HalfwordDataTransferExecutionResult,
    execute_halfword_data_transfer,
};

pub use self::instruction::ArmInstruction;

pub use self::instruction_decoder::{ArmDecodeError, decode_arm};

pub use self::multiply::{MultiplyDecodeError, MultiplyInstruction, decode_multiply};

pub use self::multiply_executor::{
    MultiplyExecutionResult, execute_multiply, multiply_iteration_count,
};

pub use self::multiply_long::{
    MultiplyLongDecodeError, MultiplyLongInstruction, decode_multiply_long,
};

pub use self::multiply_long_executor::{MultiplyLongExecutionResult, execute_multiply_long};

pub use self::single_data_transfer::{
    SingleDataTransferDecodeError, SingleDataTransferInstruction, TransferOffset,
    decode_single_data_transfer,
};

pub use self::single_data_transfer_executor::{
    SingleDataTransferExecutionError, SingleDataTransferExecutionResult,
    execute_single_data_transfer,
};

pub use self::status_register::{
    ProgramStatusRegister, StatusRegisterDecodeError, StatusRegisterInstruction,
    StatusRegisterMask, StatusRegisterOperand, decode_status_register,
};

pub use self::status_register_executor::{StatusRegisterExecutionError, execute_status_register};
