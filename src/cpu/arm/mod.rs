mod alu;
mod barrel_shifter;
mod data_processing;
mod data_processing_executor;
mod decoder;

pub use self::alu::{AddResult, AluFlags, add_with_carry, arithmetic_shift_flags, logical_flags};

pub use self::barrel_shifter::{
    ShiftResult, expand_rotated_immediate, shift_immediate, shift_register,
};

pub use self::data_processing::{
    DataProcessingDecodeError, DataProcessingInstruction, DataProcessingOpcode, Operand2,
    RegisterShift, ShiftAmount, ShiftType, decode_data_processing,
};

pub use self::data_processing_executor::{DataProcessingExecutionError, execute_data_processing};

pub use self::decoder::{ArmCondition, ArmInstructionKind, classify, condition, condition_passed};
