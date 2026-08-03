use crate::{bus::Bus, cpu::Registers};

use super::{
    ArmInstruction, BranchExchangeExecutionError, DataProcessingExecutionError,
    HalfwordDataTransferExecutionError, SingleDataTransferExecutionError, execute_branch,
    execute_branch_exchange, execute_data_processing, execute_halfword_data_transfer,
    execute_multiply, execute_single_data_transfer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmExecutionError {
    DataProcessing(DataProcessingExecutionError),

    BranchExchange(BranchExchangeExecutionError),

    HalfwordDataTransfer(HalfwordDataTransferExecutionError),

    SingleDataTransfer(SingleDataTransferExecutionError),

    UnimplementedInstruction,
}

pub fn execute_arm(
    registers: &mut Registers,
    bus: &mut Bus,
    instruction: &ArmInstruction,
    instruction_address: u32,
) -> Result<(), ArmExecutionError> {
    match instruction {
        ArmInstruction::DataProcessing(instruction) => {
            execute_data_processing(registers, *instruction)
                .map_err(ArmExecutionError::DataProcessing)
        }

        ArmInstruction::Branch(instruction) => {
            execute_branch(registers, *instruction, instruction_address);

            Ok(())
        }

        ArmInstruction::BranchExchange(instruction) => {
            execute_branch_exchange(registers, *instruction)
                .map(|_| ())
                .map_err(ArmExecutionError::BranchExchange)
        }

        ArmInstruction::HalfwordDataTransfer(instruction) => {
            execute_halfword_data_transfer(registers, bus, *instruction, instruction_address)
                .map(|_| ())
                .map_err(ArmExecutionError::HalfwordDataTransfer)
        }

        ArmInstruction::Multiply(instruction) => {
            execute_multiply(registers, *instruction);

            Ok(())
        }

        ArmInstruction::SingleDataTransfer(instruction) => {
            execute_single_data_transfer(registers, bus, *instruction, instruction_address)
                .map(|_| ())
                .map_err(ArmExecutionError::SingleDataTransfer)
        }

        _ => Err(ArmExecutionError::UnimplementedInstruction),
    }
}
