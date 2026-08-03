use super::{
    ArmCondition, BlockDataTransferInstruction, BranchExchangeInstruction, BranchInstruction,
    DataProcessingInstruction, HalfwordDataTransferInstruction, MultiplyInstruction,
    MultiplyLongInstruction, SingleDataTransferInstruction, SoftwareInterruptInstruction,
    StatusRegisterInstruction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstruction {
    BlockDataTransfer(BlockDataTransferInstruction),

    BranchExchange(BranchExchangeInstruction),

    DataProcessing(DataProcessingInstruction),

    HalfwordDataTransfer(HalfwordDataTransferInstruction),

    Multiply(MultiplyInstruction),

    MultiplyLong(MultiplyLongInstruction),

    SingleDataTransfer(SingleDataTransferInstruction),

    SoftwareInterrupt(SoftwareInterruptInstruction),

    StatusRegister(StatusRegisterInstruction),

    SingleDataSwap { condition: ArmCondition, raw: u32 },

    Branch(BranchInstruction),

    CoprocessorDataTransfer { condition: ArmCondition, raw: u32 },

    CoprocessorDataOperation { condition: ArmCondition, raw: u32 },

    CoprocessorRegisterTransfer { condition: ArmCondition, raw: u32 },

    Undefined { condition: ArmCondition, raw: u32 },
}

impl ArmInstruction {
    pub const fn condition(self) -> ArmCondition {
        match self {
            Self::BlockDataTransfer(instruction) => instruction.condition,

            Self::Branch(instruction) => instruction.condition,

            Self::BranchExchange(instruction) => instruction.condition,

            Self::DataProcessing(instruction) => instruction.condition,

            Self::HalfwordDataTransfer(instruction) => instruction.condition,

            Self::Multiply(instruction) => instruction.condition,

            Self::MultiplyLong(instruction) => instruction.condition,

            Self::SingleDataTransfer(instruction) => instruction.condition,

            Self::SoftwareInterrupt(instruction) => instruction.condition,

            Self::StatusRegister(instruction) => instruction.condition(),

            Self::SingleDataSwap { condition, .. }
            | Self::CoprocessorDataTransfer { condition, .. }
            | Self::CoprocessorDataOperation { condition, .. }
            | Self::CoprocessorRegisterTransfer { condition, .. }
            | Self::Undefined { condition, .. } => condition,
        }
    }
}
