use super::{
    ArmCondition, BlockDataTransferInstruction, BranchExchangeInstruction, BranchInstruction,
    DataProcessingInstruction, HalfwordDataTransferInstruction, MultiplyInstruction,
    MultiplyLongInstruction, SingleDataTransferInstruction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstruction {
    BlockDataTransfer(BlockDataTransferInstruction),
    BranchExchange(BranchExchangeInstruction),
    DataProcessing(DataProcessingInstruction),

    Multiply(MultiplyInstruction),

    MultiplyLong(MultiplyLongInstruction),

    SingleDataTransfer(SingleDataTransferInstruction),

    HalfwordDataTransfer(HalfwordDataTransferInstruction),

    SingleDataSwap {
        condition: ArmCondition,
        raw: u32,
    },

    Branch(BranchInstruction),

    CoprocessorDataTransfer {
        condition: ArmCondition,
        raw: u32,
    },

    CoprocessorDataOperation {
        condition: ArmCondition,
        raw: u32,
    },

    CoprocessorRegisterTransfer {
        condition: ArmCondition,
        raw: u32,
    },

    SoftwareInterrupt {
        condition: ArmCondition,
        comment: u32,
    },

    Undefined {
        condition: ArmCondition,
        raw: u32,
    },
}

impl ArmInstruction {
    pub const fn condition(self) -> ArmCondition {
        match self {
            Self::BlockDataTransfer(instruction) => instruction.condition,

            Self::Branch(instruction) => instruction.condition,

            Self::BranchExchange(instruction) => instruction.condition,

            Self::DataProcessing(instruction) => instruction.condition,

            Self::Multiply(instruction) => instruction.condition,

            Self::MultiplyLong(instruction) => instruction.condition,

            Self::SingleDataTransfer(instruction) => instruction.condition,

            Self::HalfwordDataTransfer(instruction) => instruction.condition,

            Self::SingleDataSwap { condition, .. }
            | Self::CoprocessorDataTransfer { condition, .. }
            | Self::CoprocessorDataOperation { condition, .. }
            | Self::CoprocessorRegisterTransfer { condition, .. }
            | Self::SoftwareInterrupt { condition, .. }
            | Self::Undefined { condition, .. } => condition,
        }
    }
}
