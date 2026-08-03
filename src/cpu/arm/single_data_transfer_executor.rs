use crate::{bus::Bus, cpu::Registers};

use super::{SingleDataTransferInstruction, TransferOffset, shift_immediate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleDataTransferExecutionResult {
    pub address: u32,
    pub loaded_value: Option<u32>,
    pub written_back_value: Option<u32>,
    pub branch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleDataTransferExecutionError {
    ForceUserModeNotImplemented,
    WriteBackToProgramCounter,
    LoadWriteBackBaseEqualsDestination,
    ByteLoadToProgramCounter,
}

pub fn execute_single_data_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    instruction: SingleDataTransferInstruction,
    instruction_address: u32,
) -> Result<SingleDataTransferExecutionResult, SingleDataTransferExecutionError> {
    if instruction.force_user_mode {
        /*
         * LDRT/STRT require operating-mode and privilege-aware
         * memory access, which is not implemented yet.
         */
        return Err(SingleDataTransferExecutionError::ForceUserModeNotImplemented);
    }

    let effective_write_back = instruction.write_back;

    if effective_write_back && instruction.rn as usize == Registers::PC {
        return Err(SingleDataTransferExecutionError::WriteBackToProgramCounter);
    }

    if instruction.load && effective_write_back && instruction.rn == instruction.rd {
        /*
         * LDR with write-back and Rn == Rd is architecturally
         * problematic/unpredictable. Reject it for now.
         */
        return Err(SingleDataTransferExecutionError::LoadWriteBackBaseEqualsDestination);
    }

    if instruction.load && instruction.byte && instruction.rd as usize == Registers::PC {
        return Err(SingleDataTransferExecutionError::ByteLoadToProgramCounter);
    }

    let base = read_base_register(registers, instruction.rn, instruction_address);

    let offset = evaluate_offset(registers, instruction.offset, registers.cpsr().carry());

    let offset_address = if instruction.add_offset {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };

    let transfer_address = if instruction.pre_index {
        offset_address
    } else {
        base
    };

    /*
     * Read store source before write-back, so STR Rn,[Rn],#offset
     * stores the original base value.
     *
     * For ARM STR with Rd=PC, the stored value is PC+12.
     */
    let store_value = if instruction.load {
        None
    } else {
        Some(read_store_register(
            registers,
            instruction.rd,
            instruction_address,
        ))
    };

    let mut loaded_value = None;
    let mut branch = false;

    if instruction.load {
        let value = if instruction.byte {
            bus.read8(transfer_address) as u32
        } else {
            read_unaligned_word(bus, transfer_address)
        };

        loaded_value = Some(value);

        if instruction.rd as usize == Registers::PC {
            /*
             * ARMv4 LDR PC does not switch ARM/Thumb state.
             * Force word alignment for the new ARM PC.
             */
            registers.set_pc(value & !3);
            branch = true;
        } else {
            registers.write(instruction.rd as usize, value);
        }
    } else {
        let value = store_value.expect("store value must exist for STR");

        if instruction.byte {
            bus.write8(transfer_address, value as u8);
        } else {
            /*
             * Misaligned word stores are forced down to a
             * word-aligned address.
             */
            bus.write32(transfer_address & !3, value);
        }
    }

    let written_back_value = if effective_write_back {
        registers.write(instruction.rn as usize, offset_address);

        Some(offset_address)
    } else {
        None
    };

    Ok(SingleDataTransferExecutionResult {
        address: transfer_address,
        loaded_value,
        written_back_value,
        branch,
    })
}

fn read_base_register(registers: &Registers, register: u8, instruction_address: u32) -> u32 {
    if register as usize == Registers::PC {
        instruction_address.wrapping_add(8)
    } else {
        registers.read(register as usize)
    }
}

fn read_store_register(registers: &Registers, register: u8, instruction_address: u32) -> u32 {
    if register as usize == Registers::PC {
        instruction_address.wrapping_add(12)
    } else {
        registers.read(register as usize)
    }
}

fn evaluate_offset(registers: &Registers, offset: TransferOffset, old_carry: bool) -> u32 {
    match offset {
        TransferOffset::Immediate(value) => value as u32,

        TransferOffset::ShiftedRegister {
            rm,
            shift_type,
            shift_amount,
        } => {
            let value = registers.read(rm as usize);

            shift_immediate(value, shift_type, shift_amount, old_carry).value
        }
    }
}

fn read_unaligned_word(bus: &Bus, address: u32) -> u32 {
    let aligned_value = bus.read32(address & !3);

    let rotation = (address & 3) * 8;

    aligned_value.rotate_right(rotation)
}

#[cfg(test)]
mod tests {
    use super::{SingleDataTransferExecutionError, execute_single_data_transfer};

    use crate::{
        bus::Bus,
        cpu::{Registers, arm::decode_single_data_transfer},
    };

    fn execute(registers: &mut Registers, bus: &mut Bus, raw: u32, instruction_address: u32) {
        let instruction = decode_single_data_transfer(raw).unwrap();

        execute_single_data_transfer(registers, bus, instruction, instruction_address).unwrap();
    }

    #[test]
    fn executes_str_word() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x1234_5678);
        registers.write(1, 0x0200_0100);

        // STR R0, [R1]
        execute(&mut registers, &mut bus, 0xE581_0000, 0x0800_0000);

        assert_eq!(bus.read32(0x0200_0100), 0x1234_5678);
    }

    #[test]
    fn executes_ldr_word() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_0100, 0x89AB_CDEF);

        registers.write(1, 0x0200_0100);

        // LDR R0, [R1]
        execute(&mut registers, &mut bus, 0xE591_0000, 0x0800_0000);

        assert_eq!(registers.read(0), 0x89AB_CDEF);
    }

    #[test]
    fn executes_strb() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x1234_56AB);
        registers.write(1, 0x0200_0101);

        // STRB R0, [R1]
        execute(&mut registers, &mut bus, 0xE5C1_0000, 0x0800_0000);

        assert_eq!(bus.read8(0x0200_0101), 0xAB);
    }

    #[test]
    fn ldrb_zero_extends() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write8(0x0200_0100, 0xFE);
        registers.write(1, 0x0200_0100);

        // LDRB R0, [R1]
        execute(&mut registers, &mut bus, 0xE5D1_0000, 0x0800_0000);

        assert_eq!(registers.read(0), 0xFE);
    }

    #[test]
    fn pre_index_uses_modified_address() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_0104, 0xCAFE_BABE);

        registers.write(1, 0x0200_0100);

        // LDR R0, [R1, #4]
        execute(&mut registers, &mut bus, 0xE591_0004, 0x0800_0000);

        assert_eq!(registers.read(0), 0xCAFE_BABE);

        assert_eq!(registers.read(1), 0x0200_0100);
    }

    #[test]
    fn pre_index_writeback_updates_base() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_0104, 0xCAFE_BABE);

        registers.write(1, 0x0200_0100);

        // LDR R0, [R1, #4]!
        execute(&mut registers, &mut bus, 0xE5B1_0004, 0x0800_0000);

        assert_eq!(registers.read(1), 0x0200_0104);
    }

    #[test]
    fn post_index_transfers_before_writeback() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_0100, 0xDEAD_BEEF);

        registers.write(1, 0x0200_0100);

        // LDR R0, [R1], #4
        execute(&mut registers, &mut bus, 0xE491_0004, 0x0800_0000);

        assert_eq!(registers.read(0), 0xDEAD_BEEF);

        assert_eq!(registers.read(1), 0x0200_0104);
    }

    #[test]
    fn subtracts_offset_when_u_is_clear() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_00FC, 0x1122_3344);

        registers.write(1, 0x0200_0100);

        // LDR R0, [R1, #-4]
        execute(&mut registers, &mut bus, 0xE511_0004, 0x0800_0000);

        assert_eq!(registers.read(0), 0x1122_3344);
    }

    #[test]
    fn shifted_register_offset_is_evaluated() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        /*
         * R2=1, LSL #2 -> offset 4.
         */
        registers.write(1, 0x0200_0100);
        registers.write(2, 1);

        bus.write32(0x0200_0104, 0xAABB_CCDD);

        // LDR R0, [R1, R2, LSL #2]
        execute(&mut registers, &mut bus, 0xE791_0102, 0x0800_0000);

        assert_eq!(registers.read(0), 0xAABB_CCDD);
    }

    #[test]
    fn unaligned_word_load_rotates_value() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        bus.write32(0x0200_0100, 0x4433_2211);

        registers.write(1, 0x0200_0101);

        // LDR R0, [R1]
        execute(&mut registers, &mut bus, 0xE591_0000, 0x0800_0000);

        assert_eq!(registers.read(0), 0x1144_3322);
    }

    #[test]
    fn misaligned_word_store_is_forced_aligned() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x1234_5678);
        registers.write(1, 0x0200_0102);

        // STR R0, [R1]
        execute(&mut registers, &mut bus, 0xE581_0000, 0x0800_0000);

        assert_eq!(bus.read32(0x0200_0100), 0x1234_5678);
    }

    #[test]
    fn supports_pc_relative_literal_load() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        /*
         * Instruction at 0x08000000.
         * Architectural base PC = 0x08000008.
         * Offset = 4.
         * Address = 0x0800000C.
         */
        let mut rom = vec![0; 0x20];

        rom[0x0C..0x10].copy_from_slice(&0xCAFE_BABEu32.to_le_bytes());

        bus.load_rom(&rom).unwrap();

        // LDR R0, [PC, #4]
        execute(&mut registers, &mut bus, 0xE59F_0004, 0x0800_0000);

        assert_eq!(registers.read(0), 0xCAFE_BABE);
    }

    #[test]
    fn ldr_pc_changes_control_flow() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(1, 0x0200_0100);

        bus.write32(0x0200_0100, 0x0800_0123);

        // LDR PC, [R1]
        execute(&mut registers, &mut bus, 0xE591_F000, 0x0800_0000);

        assert_eq!(registers.pc(), 0x0800_0120);

        /*
         * ARMv4 LDR PC leaves T unchanged.
         */
        assert!(!registers.cpsr().thumb_state());
    }

    #[test]
    fn str_pc_stores_instruction_address_plus_twelve() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(1, 0x0200_0100);

        // STR PC, [R1]
        execute(&mut registers, &mut bus, 0xE581_F000, 0x0800_0000);

        assert_eq!(bus.read32(0x0200_0100), 0x0800_000C);
    }

    #[test]
    fn rejects_ldrt_until_cpu_modes_exist() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        let instruction = decode_single_data_transfer(0xE4B1_0004).unwrap();

        let result =
            execute_single_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000);

        assert_eq!(
            result,
            Err(SingleDataTransferExecutionError::ForceUserModeNotImplemented)
        );
    }

    #[test]
    fn rejects_load_writeback_when_base_equals_destination() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        // LDR R1, [R1, #4]!
        let instruction = decode_single_data_transfer(0xE5B1_1004).unwrap();

        let result =
            execute_single_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000);

        assert_eq!(
            result,
            Err(SingleDataTransferExecutionError::LoadWriteBackBaseEqualsDestination)
        );
    }
}
