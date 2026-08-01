use super::cpsr::Cpsr;

#[derive(Debug, Clone)]
pub struct Registers {
    /// ARM register R0-R15
    ///
    /// R13: Stack Pointer
    /// R14: Link Register
    /// R15: Program Counter
    general: [u32; 16],

    /// CPSR - Current Program Status Register
    cpsr: Cpsr,
}

impl Registers {
    pub const SP: usize = 13;
    pub const LR: usize = 14;
    pub const PC: usize = 15;

    pub fn new() -> Self {
        Self {
            general: [0; 16],
            cpsr: Cpsr::new(),
        }
    }

    pub fn read(&self, index: usize) -> u32 {
        self.general[index]
    }

    pub fn write(&mut self, index: usize, value: u32) {
        self.general[index] = value
    }

    pub fn pc(&self) -> u32 {
        self.read(Self::PC)
    }

    pub fn set_pc(&mut self, value: u32) {
        self.write(Self::PC, value)
    }

    pub fn cpsr(&self) -> Cpsr {
        self.cpsr
    }

    pub fn cpsr_mut(&mut self) -> &mut Cpsr {
        &mut self.cpsr
    }

    pub fn set_cpsr(&mut self, value: u32) {
        self.cpsr.set_raw(value);
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Registers;

    #[test]
    fn registers_are_zero_initialized() {
        let registers = Registers::new();

        for index in 0..16 {
            assert_eq!(registers.read(index), 0);
        }

        assert_eq!(registers.cpsr().raw(), 0);
    }

    #[test]
    fn can_read_and_write_program_counter() {
        let mut registers = Registers::new();

        registers.set_pc(0x0800_0000);

        assert_eq!(registers.pc(), 0x0800_0000);
        assert_eq!(registers.read(Registers::PC), 0x0800_0000);
    }
}
