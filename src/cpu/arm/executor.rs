use crate::{
    bus::Bus,
    cpu::{ExceptionError, Registers},
};

use super::{
    ArmExecutionResult, ArmInstruction, BlockDataTransferExecutionError,
    BranchExchangeExecutionError, DataProcessingExecutionError, HalfwordDataTransferExecutionError,
    SingleDataTransferExecutionError, StatusRegisterExecutionError, execute_block_data_transfer,
    execute_branch, execute_branch_exchange, execute_data_processing,
    execute_halfword_data_transfer, execute_multiply, execute_multiply_long,
    execute_single_data_transfer, execute_software_interrupt, execute_status_register,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmExecutionError {
    BlockDataTransfer(BlockDataTransferExecutionError),

    BranchExchange(BranchExchangeExecutionError),

    DataProcessing(DataProcessingExecutionError),

    Exception(ExceptionError),

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
) -> Result<ArmExecutionResult, ArmExecutionError> {
    /*
     * Cpu::step_arm() advances PC to instruction_address + 4 before
     * calling this dispatcher.
     *
     * For executors that do not explicitly return a branch flag, we
     * can detect control-flow changes by comparing PC with this value
     * after execution.
     */
    let expected_sequential_pc = instruction_address.wrapping_add(4);

    match instruction {
        ArmInstruction::BlockDataTransfer(instruction) => {
            let result =
                execute_block_data_transfer(registers, bus, *instruction, instruction_address)
                    .map_err(ArmExecutionError::BlockDataTransfer)?;

            Ok(ArmExecutionResult {
                cycles: result.cycles,
                branch: result.branch,
            })
        }

        ArmInstruction::Branch(instruction) => {
            execute_branch(registers, *instruction, instruction_address);

            /*
             * B and BL always modify control flow.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: true,
            })
        }

        ArmInstruction::BranchExchange(instruction) => {
            execute_branch_exchange(registers, *instruction)
                .map_err(ArmExecutionError::BranchExchange)?;

            /*
             * BX always writes PC and may also switch ARM/Thumb state.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: true,
            })
        }

        ArmInstruction::DataProcessing(instruction) => {
            execute_data_processing(registers, *instruction)
                .map_err(ArmExecutionError::DataProcessing)?;

            /*
             * Most data-processing instructions are sequential.
             *
             * Some forms can write PC:
             *
             * MOV PC, Rm
             * ADD PC, ...
             * SUBS PC, LR, #4
             *
             * The latter can also perform exception return.
             */
            let branch = registers.pc() != expected_sequential_pc;

            Ok(ArmExecutionResult { cycles: 1, branch })
        }

        ArmInstruction::HalfwordDataTransfer(instruction) => {
            let result =
                execute_halfword_data_transfer(registers, bus, *instruction, instruction_address)
                    .map_err(ArmExecutionError::HalfwordDataTransfer)?;

            Ok(ArmExecutionResult {
                cycles: result.cycles,
                branch: result.branch,
            })
        }

        ArmInstruction::Multiply(instruction) => {
            execute_multiply(registers, *instruction);

            /*
             * ARM MUL/MLA cannot use PC as their destination in the
             * supported instruction model, so they remain sequential.
             *
             * Precise multiply timing will be added later.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: false,
            })
        }

        ArmInstruction::MultiplyLong(instruction) => {
            execute_multiply_long(registers, *instruction);

            /*
             * Long multiply instructions do not alter PC in the
             * supported instruction model.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: false,
            })
        }

        ArmInstruction::SingleDataTransfer(instruction) => {
            let result =
                execute_single_data_transfer(registers, bus, *instruction, instruction_address)
                    .map_err(ArmExecutionError::SingleDataTransfer)?;

            Ok(ArmExecutionResult {
                cycles: result.cycles,
                branch: result.branch,
            })
        }

        ArmInstruction::SoftwareInterrupt(instruction) => {
            execute_software_interrupt(registers, *instruction, instruction_address)
                .map_err(ArmExecutionError::Exception)?;

            /*
             * SWI enters Supervisor mode and changes PC to the SWI
             * exception vector.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: true,
            })
        }

        ArmInstruction::StatusRegister(instruction) => {
            execute_status_register(registers, *instruction)
                .map_err(ArmExecutionError::StatusRegister)?;

            /*
             * MRS and MSR do not directly write PC.
             */
            Ok(ArmExecutionResult {
                cycles: 1,
                branch: false,
            })
        }

        _ => Err(ArmExecutionError::UnimplementedInstruction),
    }
}

#[cfg(test)]
mod tests {
    use super::execute_arm;

    use crate::{
        bus::Bus,
        cpu::{
            Registers,
            arm::{ArmExecutionResult, decode_arm},
        },
    };

    #[test]
    fn branch_instruction_reports_control_flow_change() {
        let mut registers = Registers::new();

        let mut bus = Bus::new();

        /*
         * B +0
         */
        let instruction = decode_arm(0xEA00_0000).unwrap();

        /*
         * Cpu::step_arm normally performs this before dispatch.
         */
        registers.set_pc(0x0200_0004);

        let result = execute_arm(&mut registers, &mut bus, &instruction, 0x0200_0000).unwrap();

        assert!(result.branch);

        assert_eq!(registers.pc(), 0x0200_0008,);
    }

    #[test]
    fn ordinary_data_processing_is_sequential() {
        let mut registers = Registers::new();

        let mut bus = Bus::new();

        /*
         * MOV R0, #42
         */
        let instruction = decode_arm(0xE3A0_002A).unwrap();

        registers.set_pc(0x0200_0004);

        let result = execute_arm(&mut registers, &mut bus, &instruction, 0x0200_0000).unwrap();

        assert!(!result.branch);
        assert_eq!(registers.read(0), 42);
    }

    #[test]
    fn data_processing_write_to_pc_reports_branch() {
        let mut registers = Registers::new();

        let mut bus = Bus::new();

        registers.write(0, 0x0800_0100);

        /*
         * MOV PC, R0
         */
        let instruction = decode_arm(0xE1A0_F000).unwrap();

        registers.set_pc(0x0200_0004);

        let result = execute_arm(&mut registers, &mut bus, &instruction, 0x0200_0000).unwrap();

        assert!(result.branch);

        assert_eq!(registers.pc(), 0x0800_0100,);
    }

    #[test]
    fn multiply_is_sequential() {
        let mut registers = Registers::new();

        let mut bus = Bus::new();

        registers.write(1, 6);
        registers.write(2, 7);

        /*
         * MUL R0, R1, R2
         */
        let instruction = decode_arm(0xE000_0291).unwrap();

        registers.set_pc(0x0200_0004);

        let result = execute_arm(&mut registers, &mut bus, &instruction, 0x0200_0000).unwrap();

        assert!(!result.branch);
        assert_eq!(registers.read(0), 42);
    }
}
