pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 160;
pub const FRAMEBUFFER_PIXEL_COUNT: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

pub type Framebuffer = [u32; FRAMEBUFFER_PIXEL_COUNT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Invalid6,
    Invalid7,
}

impl VideoMode {
    pub const fn from_bits(bits: u16) -> Self {
        match bits & 0b111 {
            0 => Self::Mode0,
            1 => Self::Mode1,
            2 => Self::Mode2,
            3 => Self::Mode3,
            4 => Self::Mode4,
            5 => Self::Mode5,
            6 => Self::Invalid6,
            7 => Self::Invalid7,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayControl {
    raw: u16,
}

impl DisplayControl {
    pub const MODE_MASK: u16 = 0b111;
    pub const CGB_MODE_MASK: u16 = 1 << 3;
    pub const PAGE_SELECT_MASK: u16 = 1 << 4;
    pub const HBLANK_INTERVAL_FREE_MASK: u16 = 1 << 5;
    pub const OBJ_MAPPING_1D_MASK: u16 = 1 << 6;
    pub const FORCED_BLANK_MASK: u16 = 1 << 7;

    pub const BG0_ENABLE_MASK: u16 = 1 << 8;
    pub const BG1_ENABLE_MASK: u16 = 1 << 9;
    pub const BG2_ENABLE_MASK: u16 = 1 << 10;
    pub const BG3_ENABLE_MASK: u16 = 1 << 11;
    pub const OBJ_ENABLE_MASK: u16 = 1 << 12;

    pub const WINDOW0_ENABLE_MASK: u16 = 1 << 13;
    pub const WINDOW1_ENABLE_MASK: u16 = 1 << 14;
    pub const OBJ_WINDOW_ENABLE_MASK: u16 = 1 << 15;

    pub const WRITABLE_MASK: u16 = 0xFFF7;
    pub const RESET_VALUE: u16 = 0;

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

    pub const fn mode(self) -> VideoMode {
        VideoMode::from_bits(self.raw)
    }

    pub const fn page_selected(self) -> bool {
        self.raw & Self::PAGE_SELECT_MASK != 0
    }

    pub const fn forced_blank(self) -> bool {
        self.raw & Self::FORCED_BLANK_MASK != 0
    }

    pub const fn bg2_enabled(self) -> bool {
        self.raw & Self::BG2_ENABLE_MASK != 0
    }

    pub const fn bg3_enabled(self) -> bool {
        self.raw & Self::BG3_ENABLE_MASK != 0
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for DisplayControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffineBackgroundControl {
    raw: u16,
}

impl AffineBackgroundControl {
    pub const WRITABLE_MASK: u16 = 0xFFFF;

    pub const fn new() -> Self {
        Self { raw: 0 }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub fn write(&mut self, value: u16) {
        self.raw = value & Self::WRITABLE_MASK;
    }

    pub const fn priority(self) -> u8 {
        (self.raw & 0b11) as u8
    }

    pub const fn character_base_block(self) -> usize {
        ((self.raw >> 2) & 0b11) as usize
    }

    pub const fn screen_base_block(self) -> usize {
        ((self.raw >> 8) & 0x1F) as usize
    }

    pub const fn wraparound(self) -> bool {
        self.raw & (1 << 13) != 0
    }

    pub const fn size(self) -> u8 {
        ((self.raw >> 14) & 0b11) as u8
    }

    pub const fn dimension_pixels(self) -> i32 {
        128i32 << self.size()
    }
}

impl Default for AffineBackgroundControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffineBackground {
    control: AffineBackgroundControl,

    pa: i16,
    pb: i16,
    pc: i16,
    pd: i16,

    reference_x_raw: u32,
    reference_y_raw: u32,

    current_x: i32,
    current_y: i32,
}

impl AffineBackground {
    pub const fn new() -> Self {
        Self {
            control: AffineBackgroundControl::new(),
            pa: 0,
            pb: 0,
            pc: 0,
            pd: 0,
            reference_x_raw: 0,
            reference_y_raw: 0,
            current_x: 0,
            current_y: 0,
        }
    }

    pub const fn control(self) -> AffineBackgroundControl {
        self.control
    }

    pub const fn read_control(self) -> u16 {
        self.control.raw()
    }

    pub fn write_control(&mut self, value: u16) {
        self.control.write(value);
    }

    pub const fn read_pa(self) -> u16 {
        self.pa as u16
    }

    pub const fn read_pb(self) -> u16 {
        self.pb as u16
    }

    pub const fn read_pc(self) -> u16 {
        self.pc as u16
    }

    pub const fn read_pd(self) -> u16 {
        self.pd as u16
    }

    pub fn write_pa(&mut self, value: u16) {
        self.pa = value as i16;
    }

    pub fn write_pb(&mut self, value: u16) {
        self.pb = value as i16;
    }

    pub fn write_pc(&mut self, value: u16) {
        self.pc = value as i16;
    }

    pub fn write_pd(&mut self, value: u16) {
        self.pd = value as i16;
    }

    pub const fn reference_x_raw(self) -> u32 {
        self.reference_x_raw
    }

    pub const fn reference_y_raw(self) -> u32 {
        self.reference_y_raw
    }

    pub fn write_reference_x_low(&mut self, value: u16) {
        self.reference_x_raw = (self.reference_x_raw & 0x0FFF_0000) | value as u32;
        self.reload_x();
    }

    pub fn write_reference_x_high(&mut self, value: u16) {
        self.reference_x_raw =
            (self.reference_x_raw & 0x0000_FFFF) | (((value as u32) & 0x0FFF) << 16);
        self.reload_x();
    }

    pub fn write_reference_y_low(&mut self, value: u16) {
        self.reference_y_raw = (self.reference_y_raw & 0x0FFF_0000) | value as u32;
        self.reload_y();
    }

    pub fn write_reference_y_high(&mut self, value: u16) {
        self.reference_y_raw =
            (self.reference_y_raw & 0x0000_FFFF) | (((value as u32) & 0x0FFF) << 16);
        self.reload_y();
    }

    pub fn reload_reference_points(&mut self) {
        self.reload_x();
        self.reload_y();
    }

    fn reload_x(&mut self) {
        self.current_x = sign_extend_28(self.reference_x_raw);
    }

    fn reload_y(&mut self) {
        self.current_y = sign_extend_28(self.reference_y_raw);
    }

    fn advance_scanline(&mut self) {
        self.current_x = self.current_x.wrapping_add(self.pb as i32);
        self.current_y = self.current_y.wrapping_add(self.pd as i32);
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for AffineBackground {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Video {
    display_control: DisplayControl,
    bg2: AffineBackground,
    bg3: AffineBackground,
    framebuffer: Box<Framebuffer>,
    frame_ready: bool,
    frame_number: u64,
}

impl Video {
    pub const FORCED_BLANK_PIXEL: u32 = 0xFFFF_FFFF;
    pub const UNIMPLEMENTED_MODE_PIXEL: u32 = 0xFF00_0000;

    pub fn new() -> Self {
        Self {
            display_control: DisplayControl::new(),
            bg2: AffineBackground::new(),
            bg3: AffineBackground::new(),
            framebuffer: Box::new([Self::UNIMPLEMENTED_MODE_PIXEL; FRAMEBUFFER_PIXEL_COUNT]),
            frame_ready: false,
            frame_number: 0,
        }
    }

    pub const fn display_control(&self) -> DisplayControl {
        self.display_control
    }

    pub const fn read_display_control(&self) -> u16 {
        self.display_control.raw()
    }

    pub fn write_display_control(&mut self, value: u16) {
        self.display_control.write(value);
    }

    pub fn write_display_control_low(&mut self, value: u8) {
        let current = self.display_control.raw();
        self.display_control
            .write((current & 0xFF00) | value as u16);
    }

    pub fn write_display_control_high(&mut self, value: u8) {
        let current = self.display_control.raw();
        self.display_control
            .write((current & 0x00FF) | ((value as u16) << 8));
    }

    pub const fn affine_background(&self, index: usize) -> &AffineBackground {
        match index {
            2 => &self.bg2,
            3 => &self.bg3,
            _ => panic!("only BG2 and BG3 are affine in mode 2"),
        }
    }

    pub fn affine_background_mut(&mut self, index: usize) -> &mut AffineBackground {
        match index {
            2 => &mut self.bg2,
            3 => &mut self.bg3,
            _ => panic!("only BG2 and BG3 are affine in mode 2"),
        }
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.framebuffer.as_slice()
    }

    pub fn framebuffer_mut(&mut self) -> &mut [u32] {
        self.framebuffer.as_mut_slice()
    }

    pub const fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    pub fn take_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    pub const fn frame_number(&self) -> u64 {
        self.frame_number
    }

    pub fn mark_frame_ready(&mut self) {
        self.frame_ready = true;
        self.frame_number = self.frame_number.wrapping_add(1);
    }

    pub fn begin_frame(&mut self) {
        self.bg2.reload_reference_points();
        self.bg3.reload_reference_points();
    }

    pub fn render_scanline(&mut self, line: u16, vram: &[u8], palette: &[u8]) {
        let line = line as usize;

        if line >= SCREEN_HEIGHT {
            return;
        }

        if self.display_control.forced_blank() {
            self.fill_scanline(line, Self::FORCED_BLANK_PIXEL);
            self.advance_affine_scanline();
            return;
        }

        match self.display_control.mode() {
            VideoMode::Mode2 => self.render_mode2_scanline(line, vram, palette),
            VideoMode::Mode3 => self.render_mode3_scanline(line, vram),
            _ => self.fill_scanline(line, Self::UNIMPLEMENTED_MODE_PIXEL),
        }

        self.advance_affine_scanline();
    }

    fn render_mode2_scanline(&mut self, line: usize, vram: &[u8], palette: &[u8]) {
        let backdrop = read_bg_palette_color(palette, 0);
        let destination_start = line * SCREEN_WIDTH;

        for x in 0..SCREEN_WIDTH {
            let bg2_pixel = if self.display_control.bg2_enabled() {
                sample_affine_background(&self.bg2, x, vram, palette)
            } else {
                None
            };

            let bg3_pixel = if self.display_control.bg3_enabled() {
                sample_affine_background(&self.bg3, x, vram, palette)
            } else {
                None
            };

            let color = match (bg2_pixel, bg3_pixel) {
                (Some(bg2), Some(bg3)) => {
                    /*
                     * Smaller priority number appears on top.
                     * On equal priority, BG2 wins over BG3.
                     */
                    if self.bg2.control.priority() <= self.bg3.control.priority() {
                        bg2
                    } else {
                        bg3
                    }
                }

                (Some(bg2), None) => bg2,
                (None, Some(bg3)) => bg3,
                (None, None) => backdrop,
            };

            self.framebuffer[destination_start + x] = color;
        }
    }

    fn render_mode3_scanline(&mut self, line: usize, vram: &[u8]) {
        let source_line_start = line * SCREEN_WIDTH * 2;
        let destination_line_start = line * SCREEN_WIDTH;

        for x in 0..SCREEN_WIDTH {
            let source_offset = source_line_start + x * 2;
            let low = vram.get(source_offset).copied().unwrap_or(0);
            let high = vram.get(source_offset + 1).copied().unwrap_or(0);
            let color = u16::from_le_bytes([low, high]);

            self.framebuffer[destination_line_start + x] = bgr555_to_rgba8888(color);
        }
    }

    fn advance_affine_scanline(&mut self) {
        self.bg2.advance_scanline();
        self.bg3.advance_scanline();
    }

    fn fill_scanline(&mut self, line: usize, color: u32) {
        let start = line * SCREEN_WIDTH;
        self.framebuffer[start..start + SCREEN_WIDTH].fill(color);
    }

    pub fn clear_framebuffer(&mut self, color: u32) {
        self.framebuffer.fill(color);
    }

    pub fn reset(&mut self) {
        self.display_control.reset();
        self.bg2.reset();
        self.bg3.reset();
        self.framebuffer.fill(Self::UNIMPLEMENTED_MODE_PIXEL);
        self.frame_ready = false;
        self.frame_number = 0;
    }
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

fn sample_affine_background(
    background: &AffineBackground,
    screen_x: usize,
    vram: &[u8],
    palette: &[u8],
) -> Option<u32> {
    /*
     * PA/PB/PC/PD are signed 8.8 fixed point.
     * BGxX/BGxY and current reference points are signed 20.8 fixed point.
     */
    let texture_x_fixed = background
        .current_x
        .wrapping_add((background.pa as i32).wrapping_mul(screen_x as i32));

    let texture_y_fixed = background
        .current_y
        .wrapping_add((background.pc as i32).wrapping_mul(screen_x as i32));

    let mut texture_x = texture_x_fixed >> 8;
    let mut texture_y = texture_y_fixed >> 8;

    let dimension = background.control.dimension_pixels();

    if background.control.wraparound() {
        texture_x = texture_x.rem_euclid(dimension);
        texture_y = texture_y.rem_euclid(dimension);
    } else if texture_x < 0 || texture_y < 0 || texture_x >= dimension || texture_y >= dimension {
        return None;
    }

    let tiles_per_row = (dimension as usize) / 8;
    let tile_x = texture_x as usize / 8;
    let tile_y = texture_y as usize / 8;

    let map_base = background.control.screen_base_block() * 0x800;
    let map_offset = map_base + tile_y * tiles_per_row + tile_x;
    let tile_number = vram.get(map_offset).copied().unwrap_or(0) as usize;

    let character_base = background.control.character_base_block() * 0x4000;
    let pixel_x = texture_x as usize & 7;
    let pixel_y = texture_y as usize & 7;

    let tile_offset = character_base + tile_number * 64 + pixel_y * 8 + pixel_x;
    let palette_index = vram.get(tile_offset).copied().unwrap_or(0);

    if palette_index == 0 {
        return None;
    }

    Some(read_bg_palette_color(palette, palette_index))
}

fn read_bg_palette_color(palette: &[u8], index: u8) -> u32 {
    let offset = index as usize * 2;
    let low = palette.get(offset).copied().unwrap_or(0);
    let high = palette.get(offset + 1).copied().unwrap_or(0);

    bgr555_to_rgba8888(u16::from_le_bytes([low, high]))
}

const fn sign_extend_28(value: u32) -> i32 {
    ((value & 0x0FFF_FFFF) << 4) as i32 >> 4
}

pub const fn bgr555_to_rgba8888(color: u16) -> u32 {
    let red5 = (color & 0x001F) as u32;
    let green5 = ((color >> 5) & 0x001F) as u32;
    let blue5 = ((color >> 10) & 0x001F) as u32;

    let red8 = (red5 << 3) | (red5 >> 2);
    let green8 = (green5 << 3) | (green5 >> 2);
    let blue8 = (blue5 << 3) | (blue5 >> 2);

    0xFF00_0000 | (red8 << 16) | (green8 << 8) | blue8
}

#[cfg(test)]
mod tests {
    use super::{
        DisplayControl, SCREEN_HEIGHT, SCREEN_WIDTH, Video, VideoMode, bgr555_to_rgba8888,
    };

    #[test]
    fn display_control_decodes_mode() {
        let mut control = DisplayControl::new();
        control.write(3);
        assert_eq!(control.mode(), VideoMode::Mode3);
    }

    #[test]
    fn converts_bgr555_primary_colors() {
        assert_eq!(bgr555_to_rgba8888(0x001F), 0xFFFF_0000);
        assert_eq!(bgr555_to_rgba8888(0x03E0), 0xFF00_FF00);
        assert_eq!(bgr555_to_rgba8888(0x7C00), 0xFF00_00FF);
        assert_eq!(bgr555_to_rgba8888(0x7FFF), 0xFFFF_FFFF);
        assert_eq!(bgr555_to_rgba8888(0x0000), 0xFF00_0000);
    }

    #[test]
    fn mode3_renders_scanline() {
        let mut video = Video::new();
        video.write_display_control(3);

        let mut vram = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 2];
        let palette = vec![0u8; 0x400];

        vram[0..2].copy_from_slice(&0x001Fu16.to_le_bytes());
        vram[2..4].copy_from_slice(&0x03E0u16.to_le_bytes());
        vram[4..6].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
        assert_eq!(video.framebuffer()[1], 0xFF00_FF00);
        assert_eq!(video.framebuffer()[2], 0xFF00_00FF);
    }

    #[test]
    fn mode2_renders_identity_affine_tile() {
        let mut video = Video::new();

        /*
         * Mode 2, BG2 enabled.
         */
        video.write_display_control(2 | (1 << 10));

        let bg2 = video.affine_background_mut(2);

        /*
         * Character base block 0, screen base block 1, size 128x128.
         */
        bg2.write_control(1 << 8);
        bg2.write_pa(0x0100);
        bg2.write_pd(0x0100);

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];

        /*
         * Map entry 0 points to tile 1.
         */
        vram[0x800] = 1;

        /*
         * Tile 1, pixel (0,0) uses palette index 1.
         */
        vram[64] = 1;
        palette[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.begin_frame();
        video.render_scanline(0, &vram, &palette);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn mode2_palette_zero_is_transparent_and_uses_backdrop() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 10));

        let bg2 = video.affine_background_mut(2);
        bg2.write_control(1 << 8);
        bg2.write_pa(0x0100);
        bg2.write_pd(0x0100);

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];

        vram[0x800] = 1;
        vram[64] = 0;

        palette[0..2].copy_from_slice(&0x03E0u16.to_le_bytes());

        video.begin_frame();
        video.render_scanline(0, &vram, &palette);

        assert_eq!(video.framebuffer()[0], 0xFF00_FF00);
    }

    #[test]
    fn mode2_wraps_coordinates_when_overflow_is_enabled() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 10));

        let bg2 = video.affine_background_mut(2);

        /*
         * Screen base 1, overflow/wrap bit 13.
         */
        bg2.write_control((1 << 8) | (1 << 13));
        bg2.write_pa(0x0100);
        bg2.write_pd(0x0100);

        /*
         * X = -1 pixel in 20.8 fixed point.
         */
        bg2.write_reference_x_low(0xFF00);
        bg2.write_reference_x_high(0x0FFF);

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];

        /*
         * 128x128 map: coordinate 127 belongs to tile column 15.
         */
        vram[0x800 + 15] = 1;
        vram[64 + 7] = 1;
        palette[2..4].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette);

        assert_eq!(video.framebuffer()[0], 0xFF00_00FF);
    }

    #[test]
    fn mode2_composes_bg2_over_bg3_on_equal_priority() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 10) | (1 << 11));

        {
            let bg2 = video.affine_background_mut(2);
            bg2.write_control(1 << 8);
            bg2.write_pa(0x0100);
            bg2.write_pd(0x0100);
        }

        {
            let bg3 = video.affine_background_mut(3);
            bg3.write_control(2 << 8);
            bg3.write_pa(0x0100);
            bg3.write_pd(0x0100);
        }

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];

        vram[0x800] = 1;
        vram[0x1000] = 2;
        vram[64] = 1;
        vram[128] = 2;

        palette[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());
        palette[4..6].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.begin_frame();
        video.render_scanline(0, &vram, &palette);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn forced_blank_outputs_white() {
        let mut video = Video::new();
        video.write_display_control(3 | (1 << 7));

        let vram = vec![0u8; SCREEN_WIDTH * 2];
        let palette = vec![0u8; 0x400];

        video.render_scanline(0, &vram, &palette);

        assert!(
            video.framebuffer()[0..SCREEN_WIDTH]
                .iter()
                .all(|&pixel| pixel == Video::FORCED_BLANK_PIXEL)
        );
    }
}
