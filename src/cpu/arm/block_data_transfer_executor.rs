use crate::{bus::Bus, cpu::Registers};

use super::{BlockAddressingMode, BlockDataTransferInstruction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDataTransferExecutionResult {
    pub first_address: u32,
    pub final_address: u32,
    pub written_back_value: Option<u32>,
    pub register_count: u32,
    pub branch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDataTransferExecutionError {
    PsrOrUserModeNotImplemented,
    ProgramCounterAsBase,
    WriteBackBaseInRegisterList,
}

pub fn execute_block_data_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    instruction: BlockDataTransferInstruction,
    instruction_address: u32,
) -> Result<BlockDataTransferExecutionResult, BlockDataTransferExecutionError> {
    if instruction.psr_or_user_mode {
        /*
         * S=1 has two possible meanings:
         *
         * - Transfer User-mode banked registers.
         * - LDM including PC restores CPSR from SPSR.
         *
         * Both require CPU modes, banked registers and SPSRs.
         */
        return Err(BlockDataTransferExecutionError::PsrOrUserModeNotImplemented);
    }

    if instruction.rn as usize == Registers::PC {
        return Err(BlockDataTransferExecutionError::ProgramCounterAsBase);
    }

    if instruction.write_back && instruction.registers.contains(instruction.rn) {
        /*
         * Base-in-list write-back behavior depends on instruction
         * direction, load/store form and register ordering.
         *
         * Reject this unpredictable/special region until it is
         * implemented and tested explicitly.
         */
        return Err(BlockDataTransferExecutionError::WriteBackBaseInRegisterList);
    }

    let base = registers.read(instruction.rn as usize);
    let register_count = instruction.registers.count();
    let transfer_size = register_count * 4;

    let (first_address, write_back_value) =
        calculate_addresses(base, transfer_size, instruction.addressing_mode);

    /*
     * Snapshot store values before modifying memory or base.
     *
     * This also makes future base-in-list handling easier to define.
     */
    let mut store_values = [0u32; 16];

    if !instruction.load {
        for register in 0u8..16 {
            if instruction.registers.contains(register) {
                store_values[register as usize] =
                    read_store_register(registers, register, instruction_address);
            }
        }
    }

    let mut address = first_address;
    let mut branch = false;

    for register in 0u8..16 {
        if !instruction.registers.contains(register) {
            continue;
        }

        if instruction.load {
            let value = bus.read32(address);

            if register as usize == Registers::PC {
                /*
                 * ARMv4 LDM loading PC retains ARM state when S=0.
                 */
                registers.set_pc(value & !3);
                branch = true;
            } else {
                registers.write(register as usize, value);
            }
        } else {
            bus.write32(address, store_values[register as usize]);
        }

        address = address.wrapping_add(4);
    }

    if instruction.write_back {
        registers.write(instruction.rn as usize, write_back_value);
    }

    Ok(BlockDataTransferExecutionResult {
        first_address,
        final_address: address.wrapping_sub(4),
        written_back_value: instruction.write_back.then_some(write_back_value),
        register_count,
        branch,
    })
}

fn calculate_addresses(base: u32, transfer_size: u32, mode: BlockAddressingMode) -> (u32, u32) {
    match mode {
        BlockAddressingMode::IncrementAfter => (base, base.wrapping_add(transfer_size)),

        BlockAddressingMode::IncrementBefore => {
            (base.wrapping_add(4), base.wrapping_add(transfer_size))
        }

        BlockAddressingMode::DecrementAfter => (
            base.wrapping_sub(transfer_size).wrapping_add(4),
            base.wrapping_sub(transfer_size),
        ),

        BlockAddressingMode::DecrementBefore => (
            base.wrapping_sub(transfer_size),
            base.wrapping_sub(transfer_size),
        ),
    }
}

fn read_store_register(registers: &Registers, register: u8, instruction_address: u32) -> u32 {
    if register as usize == Registers::PC {
        /*
         * ARM STM stores the architectural PC value.
         *
         * On ARM7TDMI this is current instruction address + 12.
         */
        instruction_address.wrapping_add(12)
    } else {
        registers.read(register as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BlockDataTransferExecutionError, calculate_addresses, execute_block_data_transfer,
    };

    use crate::{
        bus::Bus,
        cpu::{
            Registers,
            arm::{BlockAddressingMode, decode_block_data_transfer},
        },
    };

    fn execute(registers: &mut Registers, bus: &mut Bus, raw: u32, instruction_address: u32) {
        let instruction = decode_block_data_transfer(raw).unwrap();

        execute_block_data_transfer(registers, bus, instruction, instruction_address).unwrap();
    }

    #[test]
    fn calculates_increment_after_addresses() {
        assert_eq!(
            calculate_addresses(0x1000, 12, BlockAddressingMode::IncrementAfter,),
            (0x1000, 0x100C)
        );
    }

    #[test]
    fn calculates_increment_before_addresses() {
        assert_eq!(
            calculate_addresses(0x1000, 12, BlockAddressingMode::IncrementBefore,),
            (0x1004, 0x100C)
        );
    }

    #[test]
    fn calculates_decrement_after_addresses() {
        assert_eq!(
            calculate_addresses(0x1000, 12, BlockAddressingMode::DecrementAfter,),
            (0x0FF8, 0x0FF4)
        );
    }

    #[test]
    fn calculates_decrement_before_addresses() {
        assert_eq!(
            calculate_addresses(0x1000, 12, BlockAddressingMode::DecrementBefore,),
            (0x0FF4, 0x0FF4)
        );
    }

    #[test]
    fn stmia_stores_registers_in_ascending_order() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        registers.write(1, 0x1111_1111);
        registers.write(2, 0x2222_2222);
        registers.write(3, 0x3333_3333);

        /*
         * STMIA R0!, {R1-R3}
         */
        execute(&mut registers, &mut bus, 0xE8A0_000E, 0x0800_0000);

        assert_eq!(bus.read32(0x0200_0100), 0x1111_1111);

        assert_eq!(bus.read32(0x0200_0104), 0x2222_2222);

        assert_eq!(bus.read32(0x0200_0108), 0x3333_3333);

        assert_eq!(registers.read(0), 0x0200_010C);
    }

    #[test]
    fn ldmia_loads_registers_and_updates_base() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);

        bus.write32(0x0200_0100, 10);
        bus.write32(0x0200_0104, 20);
        bus.write32(0x0200_0108, 30);

        /*
         * LDMIA R0!, {R1-R3}
         */
        execute(&mut registers, &mut bus, 0xE8B0_000E, 0x0800_0000);

        assert_eq!(registers.read(1), 10);
        assert_eq!(registers.read(2), 20);
        assert_eq!(registers.read(3), 30);

        assert_eq!(registers.read(0), 0x0200_010C);
    }

    #[test]
    fn stmdb_implements_full_descending_push() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(Registers::SP, 0x0300_8000);

        registers.write(4, 0x4444_4444);
        registers.write(5, 0x5555_5555);

        registers.write(Registers::LR, 0x0800_1234);

        /*
         * STMDB SP!, {R4, R5, LR}
         *
         * Register list:
         * bit 4 | bit 5 | bit 14 = 0x4030
         */
        execute(&mut registers, &mut bus, 0xE92D_4030, 0x0800_0000);

        assert_eq!(registers.read(Registers::SP), 0x0300_7FF4);

        assert_eq!(bus.read32(0x0300_7FF4), 0x4444_4444);

        assert_eq!(bus.read32(0x0300_7FF8), 0x5555_5555);

        assert_eq!(bus.read32(0x0300_7FFC), 0x0800_1234);
    }

    #[test]
    fn ldmia_implements_full_descending_pop() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(Registers::SP, 0x0300_7FF4);

        bus.write32(0x0300_7FF4, 0x4444_4444);

        bus.write32(0x0300_7FF8, 0x5555_5555);

        bus.write32(0x0300_7FFC, 0x0800_1234);

        /*
         * LDMIA SP!, {R4, R5, PC}
         *
         * register list = 0x8030
         */
        execute(&mut registers, &mut bus, 0xE8BD_8030, 0x0200_0000);

        assert_eq!(registers.read(4), 0x4444_4444);

        assert_eq!(registers.read(5), 0x5555_5555);

        assert_eq!(registers.pc(), 0x0800_1234);

        assert_eq!(registers.read(Registers::SP), 0x0300_8000);
    }

    #[test]
    fn stm_stores_pc_as_instruction_address_plus_twelve() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);

        /*
         * STMIA R0, {PC}
         */
        execute(&mut registers, &mut bus, 0xE880_8000, 0x0800_0000);

        assert_eq!(bus.read32(0x0200_0100), 0x0800_000C);
    }

    #[test]
    fn ldm_pc_marks_control_flow_change() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write32(0x0200_0100, 0x0800_0123);

        let instruction = decode_block_data_transfer(
            /*
             * LDMIA R0, {PC}
             */
            0xE890_8000,
        )
        .unwrap();

        let result =
            execute_block_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000)
                .unwrap();

        assert!(result.branch);

        /*
         * ARMv4 S=0 keeps ARM state and word-aligns PC.
         */
        assert_eq!(registers.pc(), 0x0800_0120);

        assert!(!registers.cpsr().thumb_state());
    }

    #[test]
    fn rejects_s_bit_until_banked_registers_exist() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        /*
         * LDMIA R0!, {R1, R2}^
         */
        let instruction = decode_block_data_transfer(0xE8F0_0006).unwrap();

        let result =
            execute_block_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000);

        assert_eq!(
            result,
            Err(BlockDataTransferExecutionError::PsrOrUserModeNotImplemented)
        );
    }

    #[test]
    fn rejects_writeback_when_base_is_in_list() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        /*
         * STMIA R0!, {R0, R1}
         */
        let instruction = decode_block_data_transfer(0xE8A0_0003).unwrap();

        let result =
            execute_block_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000);

        assert_eq!(
            result,
            Err(BlockDataTransferExecutionError::WriteBackBaseInRegisterList)
        );
    }
}
