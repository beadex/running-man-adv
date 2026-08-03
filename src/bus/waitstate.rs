#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    NonSequential,
    Sequential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessWidth {
    Byte,
    Halfword,
    Word,
}

impl AccessWidth {
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Byte => 1,
            Self::Halfword => 2,
            Self::Word => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegion {
    Bios,
    Ewram,
    Iwram,
    Io,
    Palette,
    Vram,
    Oam,
    GamePakWaitState0,
    GamePakWaitState1,
    GamePakWaitState2,
    GamePakSram,
    Unmapped,
}

impl MemoryRegion {
    pub const fn decode(address: u32) -> Self {
        match address {
            0x0000_0000..=0x0000_3FFF => Self::Bios,

            0x0200_0000..=0x02FF_FFFF => Self::Ewram,

            0x0300_0000..=0x03FF_FFFF => Self::Iwram,

            0x0400_0000..=0x04FF_FFFF => Self::Io,

            0x0500_0000..=0x05FF_FFFF => Self::Palette,

            0x0600_0000..=0x06FF_FFFF => Self::Vram,

            0x0700_0000..=0x07FF_FFFF => Self::Oam,

            0x0800_0000..=0x09FF_FFFF => Self::GamePakWaitState0,

            0x0A00_0000..=0x0BFF_FFFF => Self::GamePakWaitState1,

            0x0C00_0000..=0x0DFF_FFFF => Self::GamePakWaitState2,

            0x0E00_0000..=0x0FFF_FFFF => Self::GamePakSram,

            _ => Self::Unmapped,
        }
    }

    pub const fn is_game_pak_rom(self) -> bool {
        matches!(
            self,
            Self::GamePakWaitState0 | Self::GamePakWaitState1 | Self::GamePakWaitState2
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedAccess<T> {
    pub value: T,
    pub cycles: u32,
}

impl<T> TimedAccess<T> {
    pub const fn new(value: T, cycles: u32) -> Self {
        Self { value, cycles }
    }

    pub fn map<U>(self, function: impl FnOnce(T) -> U) -> TimedAccess<U> {
        TimedAccess {
            value: function(self.value),
            cycles: self.cycles,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitControl {
    raw: u16,
}

impl WaitControl {
    /*
     * WAITCNT:
     *
     * bits 0-1   SRAM wait control
     * bits 2-3   WS0 first access
     * bit  4     WS0 second access
     * bits 5-6   WS1 first access
     * bit  7     WS1 second access
     * bits 8-9   WS2 first access
     * bit  10    WS2 second access
     * bits 11-12 PHI terminal output
     * bit  13    reserved
     * bit  14    Game Pak prefetch enable
     * bit  15    Game Boy cartridge type flag/read-only
     */
    pub const WRITABLE_MASK: u16 = 0x5FFF;

    /*
     * Power-on default:
     *
     * SRAM = 4
     * WS0  = 4/2
     * WS1  = 4/4
     * WS2  = 4/8
     * prefetch disabled
     */
    pub const RESET_VALUE: u16 = 0x0000;

    pub const fn new() -> Self {
        Self {
            raw: Self::RESET_VALUE,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub fn write(&mut self, value: u16) {
        self.raw = value & Self::WRITABLE_MASK;
    }

    pub const fn sram_cycles(self) -> u32 {
        first_access_cycles(self.raw & 0b11)
    }

    pub const fn ws0_first_cycles(self) -> u32 {
        first_access_cycles((self.raw >> 2) & 0b11)
    }

    pub const fn ws0_second_cycles(self) -> u32 {
        if self.raw & (1 << 4) != 0 { 1 } else { 2 }
    }

    pub const fn ws1_first_cycles(self) -> u32 {
        first_access_cycles((self.raw >> 5) & 0b11)
    }

    pub const fn ws1_second_cycles(self) -> u32 {
        if self.raw & (1 << 7) != 0 { 1 } else { 4 }
    }

    pub const fn ws2_first_cycles(self) -> u32 {
        first_access_cycles((self.raw >> 8) & 0b11)
    }

    pub const fn ws2_second_cycles(self) -> u32 {
        if self.raw & (1 << 10) != 0 { 1 } else { 8 }
    }

    pub const fn phi_control(self) -> u16 {
        (self.raw >> 11) & 0b11
    }

    pub const fn prefetch_enabled(self) -> bool {
        self.raw & (1 << 14) != 0
    }

    pub const fn access_cycles(self, address: u32, width: AccessWidth, kind: AccessKind) -> u32 {
        let region = MemoryRegion::decode(address);

        match region {
            MemoryRegion::Bios => {
                /*
                 * BIOS is internally 32-bit.
                 */
                1
            }

            MemoryRegion::Ewram => {
                /*
                 * EWRAM is externally connected by a 16-bit bus.
                 */
                match width {
                    AccessWidth::Byte | AccessWidth::Halfword => 3,

                    AccessWidth::Word => 6,
                }
            }

            MemoryRegion::Iwram => {
                /*
                 * IWRAM is internal and 32-bit.
                 */
                1
            }

            MemoryRegion::Io | MemoryRegion::Palette | MemoryRegion::Vram | MemoryRegion::Oam => {
                /*
                 * Preliminary 16-bit peripheral bus model.
                 */
                match width {
                    AccessWidth::Byte | AccessWidth::Halfword => 1,

                    AccessWidth::Word => 2,
                }
            }

            MemoryRegion::GamePakWaitState0 => self.game_pak_rom_cycles(
                width,
                kind,
                self.ws0_first_cycles(),
                self.ws0_second_cycles(),
            ),

            MemoryRegion::GamePakWaitState1 => self.game_pak_rom_cycles(
                width,
                kind,
                self.ws1_first_cycles(),
                self.ws1_second_cycles(),
            ),

            MemoryRegion::GamePakWaitState2 => self.game_pak_rom_cycles(
                width,
                kind,
                self.ws2_first_cycles(),
                self.ws2_second_cycles(),
            ),

            MemoryRegion::GamePakSram => {
                /*
                 * SRAM/Flash is exposed through an 8-bit bus.
                 */
                match width {
                    AccessWidth::Byte => self.sram_cycles(),

                    AccessWidth::Halfword => self.sram_cycles() * 2,

                    AccessWidth::Word => self.sram_cycles() * 4,
                }
            }

            MemoryRegion::Unmapped => 1,
        }
    }

    const fn game_pak_rom_cycles(
        self,
        width: AccessWidth,
        kind: AccessKind,
        first: u32,
        second: u32,
    ) -> u32 {
        /*
         * Game Pak ROM has a 16-bit data bus.
         *
         * Byte and halfword accesses require one bus transfer.
         * Word accesses require two halfword transfers.
         */
        match width {
            AccessWidth::Byte | AccessWidth::Halfword => match kind {
                AccessKind::NonSequential => first,
                AccessKind::Sequential => second,
            },

            AccessWidth::Word => {
                match kind {
                    /*
                     * First half is N, second half is S.
                     */
                    AccessKind::NonSequential => first + second,

                    /*
                     * Both halves continue a sequential stream.
                     */
                    AccessKind::Sequential => second + second,
                }
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for WaitControl {
    fn default() -> Self {
        Self::new()
    }
}

const fn first_access_cycles(encoded: u16) -> u32 {
    match encoded & 0b11 {
        0b00 => 4,
        0b01 => 3,
        0b10 => 2,
        0b11 => 8,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AccessKind, AccessWidth, MemoryRegion, WaitControl};

    #[test]
    fn decodes_memory_regions() {
        assert_eq!(MemoryRegion::decode(0x0000_0000), MemoryRegion::Bios,);

        assert_eq!(MemoryRegion::decode(0x0200_0000), MemoryRegion::Ewram,);

        assert_eq!(
            MemoryRegion::decode(0x0800_0000),
            MemoryRegion::GamePakWaitState0,
        );

        assert_eq!(
            MemoryRegion::decode(0x0A00_0000),
            MemoryRegion::GamePakWaitState1,
        );

        assert_eq!(
            MemoryRegion::decode(0x0C00_0000),
            MemoryRegion::GamePakWaitState2,
        );

        assert_eq!(MemoryRegion::decode(0x0E00_0000), MemoryRegion::GamePakSram,);
    }

    #[test]
    fn reset_waitstates_match_startup_values() {
        let wait = WaitControl::new();

        assert_eq!(wait.sram_cycles(), 4);

        assert_eq!(wait.ws0_first_cycles(), 4,);

        assert_eq!(wait.ws0_second_cycles(), 2,);

        assert_eq!(wait.ws1_first_cycles(), 4,);

        assert_eq!(wait.ws1_second_cycles(), 4,);

        assert_eq!(wait.ws2_first_cycles(), 4,);

        assert_eq!(wait.ws2_second_cycles(), 8,);

        assert!(!wait.prefetch_enabled());
    }

    #[test]
    fn waitcnt_fields_are_decoded() {
        let mut wait = WaitControl::new();

        let value = 0b10
            | (0b01 << 2)
            | (1 << 4)
            | (0b10 << 5)
            | (1 << 7)
            | (0b11 << 8)
            | (1 << 10)
            | (1 << 14);

        wait.write(value);

        assert_eq!(wait.sram_cycles(), 2);

        assert_eq!(wait.ws0_first_cycles(), 3,);

        assert_eq!(wait.ws0_second_cycles(), 1,);

        assert_eq!(wait.ws1_first_cycles(), 2,);

        assert_eq!(wait.ws1_second_cycles(), 1,);

        assert_eq!(wait.ws2_first_cycles(), 8,);

        assert_eq!(wait.ws2_second_cycles(), 1,);

        assert!(wait.prefetch_enabled());
    }

    #[test]
    fn ewram_word_access_uses_two_bus_units() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(0x0200_0000, AccessWidth::Word, AccessKind::NonSequential,),
            6,
        );
    }

    #[test]
    fn ws0_nonsequential_halfword_uses_first_timing() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(
                0x0800_0000,
                AccessWidth::Halfword,
                AccessKind::NonSequential,
            ),
            4,
        );
    }

    #[test]
    fn ws0_sequential_halfword_uses_second_timing() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(0x0800_0002, AccessWidth::Halfword, AccessKind::Sequential,),
            2,
        );
    }

    #[test]
    fn ws0_nonsequential_word_is_first_plus_second() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(0x0800_0000, AccessWidth::Word, AccessKind::NonSequential,),
            6,
        );
    }

    #[test]
    fn ws0_sequential_word_is_two_second_accesses() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(0x0800_0004, AccessWidth::Word, AccessKind::Sequential,),
            4,
        );
    }

    #[test]
    fn sram_width_scales_8_bit_accesses() {
        let wait = WaitControl::new();

        assert_eq!(
            wait.access_cycles(0x0E00_0000, AccessWidth::Byte, AccessKind::NonSequential,),
            4,
        );

        assert_eq!(
            wait.access_cycles(
                0x0E00_0000,
                AccessWidth::Halfword,
                AccessKind::NonSequential,
            ),
            8,
        );

        assert_eq!(
            wait.access_cycles(0x0E00_0000, AccessWidth::Word, AccessKind::NonSequential,),
            16,
        );
    }
}
