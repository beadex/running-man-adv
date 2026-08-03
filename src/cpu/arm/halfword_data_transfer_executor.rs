use crate::{bus::Bus, cpu::Registers};

use super::{HalfwordDataTransferInstruction, HalfwordTransferKind, HalfwordTransferOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfwordDataTransferExecutionResult {
    pub address: u32,
    pub loaded_value: Option<u32>,
    pub written_back_value: Option<u32>,
    pub branch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfwordDataTransferExecutionError {
    WriteBackToProgramCounter,
    LoadWriteBackBaseEqualsDestination,

    /*
     * ARM7TDMI specifies odd-address halfword access as
     * unpredictable. Reject it deterministically for now.
     */
    MisalignedHalfwordAccess,
}

pub fn execute_halfword_data_transfer(
    registers: &mut Registers,
    bus: &mut Bus,
    instruction: HalfwordDataTransferInstruction,
    instruction_address: u32,
) -> Result<HalfwordDataTransferExecutionResult, HalfwordDataTransferExecutionError> {
    if instruction.write_back && instruction.rn as usize == Registers::PC {
        return Err(HalfwordDataTransferExecutionError::WriteBackToProgramCounter);
    }

    if instruction.kind.is_load() && instruction.write_back && instruction.rn == instruction.rd {
        return Err(HalfwordDataTransferExecutionError::LoadWriteBackBaseEqualsDestination);
    }

    let base = read_base_register(registers, instruction.rn, instruction_address);

    let offset = evaluate_offset(registers, instruction.offset);

    let modified_base = if instruction.add_offset {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };

    let transfer_address = if instruction.pre_index {
        modified_base
    } else {
        base
    };

    if instruction.kind.uses_halfword() && transfer_address & 1 != 0 {
        return Err(HalfwordDataTransferExecutionError::MisalignedHalfwordAccess);
    }

    /*
     * Read the source before any base write-back.
     */
    let store_value = match instruction.kind {
        HalfwordTransferKind::StoreHalfword => Some(read_store_register(
            registers,
            instruction.rd,
            instruction_address,
        )),

        _ => None,
    };

    let mut loaded_value = None;
    let mut branch = false;

    match instruction.kind {
        HalfwordTransferKind::StoreHalfword => {
            bus.write16(
                transfer_address,
                store_value.expect("STRH source must exist") as u16,
            );
        }

        HalfwordTransferKind::LoadHalfword => {
            let value = bus.read16(transfer_address) as u32;

            loaded_value = Some(value);

            write_loaded_value(registers, instruction.rd, value, &mut branch);
        }

        HalfwordTransferKind::LoadSignedByte => {
            let value = (bus.read8(transfer_address) as i8) as i32 as u32;

            loaded_value = Some(value);

            write_loaded_value(registers, instruction.rd, value, &mut branch);
        }

        HalfwordTransferKind::LoadSignedHalfword => {
            let value = (bus.read16(transfer_address) as i16) as i32 as u32;

            loaded_value = Some(value);

            write_loaded_value(registers, instruction.rd, value, &mut branch);
        }
    }

    let written_back_value = if instruction.write_back {
        registers.write(instruction.rn as usize, modified_base);

        Some(modified_base)
    } else {
        None
    };

    Ok(HalfwordDataTransferExecutionResult {
        address: transfer_address,
        loaded_value,
        written_back_value,
        branch,
    })
}

fn evaluate_offset(registers: &Registers, offset: HalfwordTransferOffset) -> u32 {
    match offset {
        HalfwordTransferOffset::Immediate(value) => value as u32,

        HalfwordTransferOffset::Register(rm) => registers.read(rm as usize),
    }
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

fn write_loaded_value(registers: &mut Registers, destination: u8, value: u32, branch: &mut bool) {
    if destination as usize == Registers::PC {
        /*
         * ARMv4 load into PC remains in ARM state.
         */
        registers.set_pc(value & !3);
        *branch = true;
    } else {
        registers.write(destination as usize, value);
    }
}

#[cfg(test)]
mod tests {
    use super::{HalfwordDataTransferExecutionError, execute_halfword_data_transfer};

    use crate::{
        bus::Bus,
        cpu::{Registers, arm::decode_halfword_data_transfer},
    };

    fn execute(registers: &mut Registers, bus: &mut Bus, raw: u32) {
        let instruction = decode_halfword_data_transfer(raw).unwrap();

        execute_halfword_data_transfer(registers, bus, instruction, 0x0800_0000).unwrap();
    }

    #[test]
    fn executes_strh() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        registers.write(1, 0x1234_ABCD);

        // STRH R1, [R0]
        execute(&mut registers, &mut bus, 0xE1C0_10B0);

        assert_eq!(bus.read16(0x0200_0100), 0xABCD);
    }

    #[test]
    fn ldrh_zero_extends() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write16(0x0200_0100, 0xFF80);

        // LDRH R1, [R0]
        execute(&mut registers, &mut bus, 0xE1D0_10B0);

        assert_eq!(registers.read(1), 0x0000_FF80);
    }

    #[test]
    fn ldrsb_sign_extends_negative_byte() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0101);
        bus.write8(0x0200_0101, 0x80);

        // LDRSB R1, [R0]
        execute(&mut registers, &mut bus, 0xE1D0_10D0);

        assert_eq!(registers.read(1), 0xFFFF_FF80);
    }

    #[test]
    fn ldrsb_sign_extends_positive_byte() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write8(0x0200_0100, 0x7F);

        execute(&mut registers, &mut bus, 0xE1D0_10D0);

        assert_eq!(registers.read(1), 0x0000_007F);
    }

    #[test]
    fn ldrsh_sign_extends() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write16(0x0200_0100, 0x8001);

        // LDRSH R1, [R0]
        execute(&mut registers, &mut bus, 0xE1D0_10F0);

        assert_eq!(registers.read(1), 0xFFFF_8001);
    }

    #[test]
    fn applies_immediate_offset() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write16(0x0200_0104, 0xCAFE);

        // LDRH R1, [R0, #4]
        execute(&mut registers, &mut bus, 0xE1D0_10B4);

        assert_eq!(registers.read(1), 0xCAFE);
    }

    #[test]
    fn applies_register_offset() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        registers.write(2, 4);

        bus.write16(0x0200_0104, 0x1234);

        // LDRH R1, [R0, R2]
        execute(&mut registers, &mut bus, 0xE190_10B2);

        assert_eq!(registers.read(1), 0x1234);
    }

    #[test]
    fn post_index_uses_old_base_then_writes_back() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);
        bus.write16(0x0200_0100, 0xBEEF);

        // LDRH R1, [R0], #4
        execute(&mut registers, &mut bus, 0xE0D0_10B4);

        assert_eq!(registers.read(1), 0xBEEF);

        assert_eq!(registers.read(0), 0x0200_0104);
    }

    #[test]
    fn rejects_odd_halfword_load_address() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0101);

        let instruction = decode_halfword_data_transfer(0xE1D0_10B0).unwrap();

        let result =
            execute_halfword_data_transfer(&mut registers, &mut bus, instruction, 0x0800_0000);

        assert_eq!(
            result,
            Err(HalfwordDataTransferExecutionError::MisalignedHalfwordAccess)
        );
    }

    #[test]
    fn strh_pc_stores_instruction_address_plus_twelve() {
        let mut registers = Registers::new();
        let mut bus = Bus::new();

        registers.write(0, 0x0200_0100);

        // STRH PC, [R0]
        execute(&mut registers, &mut bus, 0xE1C0_F0B0);

        assert_eq!(bus.read16(0x0200_0100), 0x000C);
    }
}
