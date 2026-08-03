use super::{
    ArmCondition, BlockDataTransferInstruction, BranchExchangeInstruction, BranchInstruction,
    DataProcessingInstruction, HalfwordDataTransferInstruction, MultiplyInstruction,
    SingleDataTransferInstruction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstruction {
    DataProcessing(DataProcessingInstruction),

    BranchExchange(BranchExchangeInstruction),

    Multiply(MultiplyInstruction),

    SingleDataTransfer(SingleDataTransferInstruction),

    HalfwordDataTransfer(HalfwordDataTransferInstruction),

    BlockDataTransfer(BlockDataTransferInstruction),

    MultiplyLong {
        condition: ArmCondition,
        raw: u32,
    },

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

            Self::SingleDataTransfer(instruction) => instruction.condition,

            Self::HalfwordDataTransfer(instruction) => instruction.condition,

            Self::MultiplyLong { condition, .. }
            | Self::SingleDataSwap { condition, .. }
            | Self::CoprocessorDataTransfer { condition, .. }
            | Self::CoprocessorDataOperation { condition, .. }
            | Self::CoprocessorRegisterTransfer { condition, .. }
            | Self::SoftwareInterrupt { condition, .. }
            | Self::Undefined { condition, .. } => condition,
        }
    }
}
