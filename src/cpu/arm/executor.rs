use crate::{bus::Bus, cpu::Registers};

use super::{
    ArmInstruction, BlockDataTransferExecutionError, BranchExchangeExecutionError,
    DataProcessingExecutionError, HalfwordDataTransferExecutionError,
    SingleDataTransferExecutionError, StatusRegisterExecutionError, execute_block_data_transfer,
    execute_branch, execute_branch_exchange, execute_data_processing,
    execute_halfword_data_transfer, execute_multiply, execute_multiply_long,
    execute_single_data_transfer, execute_status_register,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmExecutionError {
    BlockDataTransfer(BlockDataTransferExecutionError),

    BranchExchange(BranchExchangeExecutionError),

    DataProcessing(DataProcessingExecutionError),

    HalfwordDataTransfer(HalfwordDataTransferExecutionError),

    SingleDataTransfer(SingleDataTransferExecutionError),

    StatusRegister(StatusRegisterExecutionError),

    UnimplementedInstruction,
}

pub fn execute_arm(
    registers: &mut Registers,
    bus: &mut Bus,
    instruction: &ArmInstruction,
    instruction_address: u32,
) -> Result<(), ArmExecutionError> {
    match instruction {
        ArmInstruction::BlockDataTransfer(instruction) => {
            execute_block_data_transfer(registers, bus, *instruction, instruction_address)
                .map(|_| ())
                .map_err(ArmExecutionError::BlockDataTransfer)
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

        ArmInstruction::DataProcessing(instruction) => {
            execute_data_processing(registers, *instruction)
                .map_err(ArmExecutionError::DataProcessing)
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

        ArmInstruction::MultiplyLong(instruction) => {
            execute_multiply_long(registers, *instruction);

            Ok(())
        }

        ArmInstruction::SingleDataTransfer(instruction) => {
            execute_single_data_transfer(registers, bus, *instruction, instruction_address)
                .map(|_| ())
                .map_err(ArmExecutionError::SingleDataTransfer)
        }

        ArmInstruction::StatusRegister(instruction) => {
            execute_status_register(registers, *instruction)
                .map_err(ArmExecutionError::StatusRegister)
        }

        _ => Err(ArmExecutionError::UnimplementedInstruction),
    }
}
