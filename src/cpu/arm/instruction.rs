use super::{
    ArmCondition, BranchExchangeInstruction, BranchInstruction, DataProcessingInstruction,
    MultiplyInstruction, SingleDataTransferInstruction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstruction {
    DataProcessing(DataProcessingInstruction),

    BranchExchange(BranchExchangeInstruction),

    Multiply(MultiplyInstruction),

    SingleDataTransfer(SingleDataTransferInstruction),

    MultiplyLong {
        condition: ArmCondition,
        raw: u32,
    },

    SingleDataSwap {
        condition: ArmCondition,
        raw: u32,
    },

    HalfwordDataTransfer {
        condition: ArmCondition,
        raw: u32,
    },

    BlockDataTransfer {
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
            Self::DataProcessing(instruction) => instruction.condition,

            Self::Branch(instruction) => instruction.condition,

            Self::BranchExchange(instruction) => instruction.condition,

            Self::Multiply(instruction) => instruction.condition,

            Self::SingleDataTransfer(instruction) => instruction.condition,

            Self::MultiplyLong { condition, .. }
            | Self::SingleDataSwap { condition, .. }
            | Self::HalfwordDataTransfer { condition, .. }
            | Self::BlockDataTransfer { condition, .. }
            | Self::CoprocessorDataTransfer { condition, .. }
            | Self::CoprocessorDataOperation { condition, .. }
            | Self::CoprocessorRegisterTransfer { condition, .. }
            | Self::SoftwareInterrupt { condition, .. }
            | Self::Undefined { condition, .. } => condition,
        }
    }
}
