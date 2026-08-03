use crate::cpu::Registers;

use super::{
    ArmInstruction, DataProcessingExecutionError, execute_branch, execute_data_processing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmExecutionError {
    DataProcessing(DataProcessingExecutionError),

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

        _ => Err(ArmExecutionError::UnimplementedInstruction),
    }
}
