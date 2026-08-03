use crate::cpu::Registers;

use super::{
    ArmInstruction, BranchExchangeExecutionError, DataProcessingExecutionError, execute_branch,
    execute_branch_exchange, execute_data_processing, execute_multiply,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmExecutionError {
    DataProcessing(DataProcessingExecutionError),

    BranchExchange(BranchExchangeExecutionError),

    UnimplementedInstruction,
}

pub fn execute_arm(
    registers: &mut Registers,
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

        ArmInstruction::Multiply(instruction) => {
            execute_multiply(registers, *instruction);

            Ok(())
        }

        _ => Err(ArmExecutionError::UnimplementedInstruction),
    }
}
