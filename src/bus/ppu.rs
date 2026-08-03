use super::InterruptSource;

const VISIBLE_LINE_WORD_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispStat {
    vblank_irq_enabled: bool,
    hblank_irq_enabled: bool,
    vcounter_irq_enabled: bool,
    vcount_setting: u8,
}

impl DispStat {
    pub const fn new() -> Self {
        Self {
            vblank_irq_enabled: false,
            hblank_irq_enabled: false,
            vcounter_irq_enabled: false,
            vcount_setting: 0,
        }
    }

    pub const fn vblank_irq_enabled(self) -> bool {
        self.vblank_irq_enabled
    }

    pub const fn hblank_irq_enabled(self) -> bool {
        self.hblank_irq_enabled
    }

    pub const fn vcounter_irq_enabled(self) -> bool {
        self.vcounter_irq_enabled
    }

    pub const fn vcount_setting(self) -> u8 {
        self.vcount_setting
    }

    pub fn write(&mut self, value: u16) {
        self.vblank_irq_enabled = value & (1 << 3) != 0;

        self.hblank_irq_enabled = value & (1 << 4) != 0;

        self.vcounter_irq_enabled = value & (1 << 5) != 0;

        self.vcount_setting = (value >> 8) as u8;
    }

    pub const fn raw(self, in_vblank: bool, in_hblank: bool, vcount_match: bool) -> u16 {
        let mut value = 0u16;

        if in_vblank {
            value |= 1 << 0;
        }

        if in_hblank {
            value |= 1 << 1;
        }

        if vcount_match {
            value |= 1 << 2;
        }

        if self.vblank_irq_enabled {
            value |= 1 << 3;
        }

        if self.hblank_irq_enabled {
            value |= 1 << 4;
        }

        if self.vcounter_irq_enabled {
            value |= 1 << 5;
        }

        value | ((self.vcount_setting as u16) << 8)
    }
}

impl Default for DispStat {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleScanlineSet {
    words: [u32; VISIBLE_LINE_WORD_COUNT],
}

impl VisibleScanlineSet {
    pub const fn new() -> Self {
        Self {
            words: [0; VISIBLE_LINE_WORD_COUNT],
        }
    }

    pub fn insert(&mut self, line: u16) {
        if line >= Ppu::VISIBLE_LINES {
            return;
        }

        let word = line as usize / 32;

        let bit = line as usize % 32;

        self.words[word] |= 1u32 << bit;
    }

    pub const fn contains(&self, line: u16) -> bool {
        if line >= Ppu::VISIBLE_LINES {
            return false;
        }

        let word = line as usize / 32;

        let bit = line as usize % 32;

        self.words[word] & (1u32 << bit) != 0
    }

    pub const fn is_empty(&self) -> bool {
        let mut index = 0;

        while index < VISIBLE_LINE_WORD_COUNT {
            if self.words[index] != 0 {
                return false;
            }

            index += 1;
        }

        true
    }

    pub fn iter(&self) -> VisibleScanlineIter<'_> {
        VisibleScanlineIter {
            set: self,
            next_line: 0,
        }
    }
}

impl Default for VisibleScanlineSet {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VisibleScanlineIter<'a> {
    set: &'a VisibleScanlineSet,
    next_line: u16,
}

impl Iterator for VisibleScanlineIter<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_line < Ppu::VISIBLE_LINES {
            let line = self.next_line;

            self.next_line += 1;

            if self.set.contains(line) {
                return Some(line);
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpuTickResult {
    pub hblank_starts: u32,
    pub vblank_starts: u32,
    pub vcount_matches: u32,
    pub new_frames: u32,

    pub interrupt_requests: u16,

    /*
     * A visible line is complete when its HBlank begins.
     */
    pub completed_visible_lines: VisibleScanlineSet,
}

impl PpuTickResult {
    pub const fn new() -> Self {
        Self {
            hblank_starts: 0,
            vblank_starts: 0,
            vcount_matches: 0,
            new_frames: 0,
            interrupt_requests: 0,
            completed_visible_lines: VisibleScanlineSet::new(),
        }
    }
}

impl Default for PpuTickResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ppu {
    line_cycle: u16,
    vcount: u16,

    in_hblank: bool,
    in_vblank: bool,
    vcount_match: bool,

    dispstat: DispStat,
}

impl Ppu {
    pub const HDRAW_CYCLES: u16 = 960;
    pub const HBLANK_CYCLES: u16 = 272;

    pub const CYCLES_PER_LINE: u16 = Self::HDRAW_CYCLES + Self::HBLANK_CYCLES;

    pub const VISIBLE_LINES: u16 = 160;
    pub const TOTAL_LINES: u16 = 228;

    pub const fn new() -> Self {
        Self {
            line_cycle: 0,
            vcount: 0,

            in_hblank: false,
            in_vblank: false,

            vcount_match: true,

            dispstat: DispStat::new(),
        }
    }

    pub const fn line_cycle(&self) -> u16 {
        self.line_cycle
    }

    pub const fn vcount(&self) -> u16 {
        self.vcount
    }

    pub const fn in_hblank(&self) -> bool {
        self.in_hblank
    }

    pub const fn in_vblank(&self) -> bool {
        self.in_vblank
    }

    pub const fn vcount_match(&self) -> bool {
        self.vcount_match
    }

    pub const fn dispstat(&self) -> DispStat {
        self.dispstat
    }

    pub const fn read_dispstat(&self) -> u16 {
        self.dispstat
            .raw(self.in_vblank, self.in_hblank, self.vcount_match)
    }

    pub fn write_dispstat(&mut self, value: u16) {
        self.dispstat.write(value);

        self.vcount_match = self.vcount as u8 == self.dispstat.vcount_setting();
    }

    pub fn tick(&mut self, cycles: u32) -> PpuTickResult {
        let mut remaining = cycles;

        let mut result = PpuTickResult::new();

        while remaining > 0 {
            let next_boundary = if self.in_hblank {
                Self::CYCLES_PER_LINE
            } else {
                Self::HDRAW_CYCLES
            };

            let until_boundary = next_boundary - self.line_cycle;

            if remaining < until_boundary as u32 {
                self.line_cycle += remaining as u16;

                break;
            }

            self.line_cycle = next_boundary;

            remaining -= until_boundary as u32;

            if self.in_hblank {
                self.finish_scanline(&mut result);
            } else {
                self.enter_hblank(&mut result);
            }
        }

        result
    }

    fn enter_hblank(&mut self, result: &mut PpuTickResult) {
        self.in_hblank = true;

        result.hblank_starts += 1;

        /*
         * The currently displayed visible line is now complete and
         * can be converted from VRAM into the host framebuffer.
         */
        if self.vcount < Self::VISIBLE_LINES {
            result.completed_visible_lines.insert(self.vcount);
        }

        if self.dispstat.hblank_irq_enabled() {
            result.interrupt_requests |= InterruptSource::HBlank.mask();
        }
    }

    fn finish_scanline(&mut self, result: &mut PpuTickResult) {
        self.line_cycle = 0;
        self.in_hblank = false;

        self.vcount += 1;

        if self.vcount == Self::VISIBLE_LINES {
            self.in_vblank = true;

            result.vblank_starts += 1;

            if self.dispstat.vblank_irq_enabled() {
                result.interrupt_requests |= InterruptSource::VBlank.mask();
            }
        }

        if self.vcount == Self::TOTAL_LINES {
            self.vcount = 0;
            self.in_vblank = false;

            result.new_frames += 1;
        }

        let previous_match = self.vcount_match;

        self.vcount_match = self.vcount as u8 == self.dispstat.vcount_setting();

        if self.vcount_match && !previous_match {
            result.vcount_matches += 1;

            if self.dispstat.vcounter_irq_enabled() {
                result.interrupt_requests |= InterruptSource::VCounterMatch.mask();
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Ppu;

    use crate::bus::InterruptSource;

    #[test]
    fn visible_scanline_completes_at_hblank_start() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::HDRAW_CYCLES as u32);

        assert!(result.completed_visible_lines.contains(0),);
    }

    #[test]
    fn vblank_lines_are_not_marked_for_rendering() {
        let mut ppu = Ppu::new();

        ppu.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        assert_eq!(ppu.vcount(), Ppu::VISIBLE_LINES,);

        let result = ppu.tick(Ppu::HDRAW_CYCLES as u32);

        assert!(result.completed_visible_lines.is_empty(),);
    }

    #[test]
    fn tick_can_complete_multiple_visible_lines() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * 3 + Ppu::HDRAW_CYCLES as u32);

        assert!(result.completed_visible_lines.contains(0),);

        assert!(result.completed_visible_lines.contains(1),);

        assert!(result.completed_visible_lines.contains(2),);

        assert!(result.completed_visible_lines.contains(3),);
    }

    #[test]
    fn vblank_irq_still_works() {
        let mut ppu = Ppu::new();

        ppu.write_dispstat(1 << 3);

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        assert_ne!(
            result.interrupt_requests & InterruptSource::VBlank.mask(),
            0,
        );
    }
}
