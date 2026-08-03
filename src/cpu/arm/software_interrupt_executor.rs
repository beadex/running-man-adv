use crate::cpu::{Exception, ExceptionEntryResult, ExceptionError, Registers, enter_exception};

use super::SoftwareInterruptInstruction;

pub fn execute_software_interrupt(
    registers: &mut Registers,
    _instruction: SoftwareInterruptInstruction,
    instruction_address: u32,
) -> Result<ExceptionEntryResult, ExceptionError> {
    /*
     * ARM SWI return address is the instruction following SWI.
     *
     * ARM instruction width is four bytes.
     */
    let return_address = instruction_address.wrapping_add(4);

    enter_exception(registers, Exception::SoftwareInterrupt, return_address)
}
