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

    pub const fn obj_enabled(self) -> bool {
        self.raw & Self::OBJ_ENABLE_MASK != 0
    }

    pub const fn obj_mapping_1d(self) -> bool {
        self.raw & Self::OBJ_MAPPING_1D_MASK != 0
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BackgroundPixel {
    color: u32,
    priority: u8,
    layer: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectPixel {
    color: u32,
    priority: u8,
    oam_index: u8,
    semi_transparent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectAttributes {
    attr0: u16,
    attr1: u16,
    attr2: u16,
}

impl ObjectAttributes {
    const fn y(self) -> u16 {
        self.attr0 & 0x00FF
    }

    const fn affine(self) -> bool {
        self.attr0 & (1 << 8) != 0
    }

    const fn double_size(self) -> bool {
        self.affine() && self.attr0 & (1 << 9) != 0
    }

    const fn disabled(self) -> bool {
        !self.affine() && self.attr0 & (1 << 9) != 0
    }

    const fn object_mode(self) -> u8 {
        ((self.attr0 >> 10) & 0b11) as u8
    }

    const fn color_8bpp(self) -> bool {
        self.attr0 & (1 << 13) != 0
    }

    const fn shape(self) -> u8 {
        ((self.attr0 >> 14) & 0b11) as u8
    }

    const fn x(self) -> u16 {
        self.attr1 & 0x01FF
    }

    const fn affine_parameter_index(self) -> usize {
        ((self.attr1 >> 9) & 0x1F) as usize
    }

    const fn horizontal_flip(self) -> bool {
        !self.affine() && self.attr1 & (1 << 12) != 0
    }

    const fn vertical_flip(self) -> bool {
        !self.affine() && self.attr1 & (1 << 13) != 0
    }

    const fn size(self) -> u8 {
        ((self.attr1 >> 14) & 0b11) as u8
    }

    const fn tile_number(self) -> usize {
        (self.attr2 & 0x03FF) as usize
    }

    const fn priority(self) -> u8 {
        ((self.attr2 >> 10) & 0b11) as u8
    }

    const fn palette_bank(self) -> usize {
        ((self.attr2 >> 12) & 0x0F) as usize
    }
}

#[derive(Debug, Clone)]
pub struct Video {
    display_control: DisplayControl,
    bg2: AffineBackground,
    bg3: AffineBackground,
    framebuffer: Box<Framebuffer>,
    object_line: Box<[Option<ObjectPixel>; SCREEN_WIDTH]>,
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
            object_line: Box::new([None; SCREEN_WIDTH]),
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

    pub fn render_scanline(&mut self, line: u16, vram: &[u8], palette: &[u8], oam: &[u8]) {
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
            VideoMode::Mode2 => self.render_mode2_scanline(line, vram, palette, oam),
            VideoMode::Mode3 => self.render_mode3_scanline(line, vram, palette, oam),
            VideoMode::Mode4 => self.render_mode4_scanline(line, vram, palette, oam),
            _ => self.fill_scanline(line, Self::UNIMPLEMENTED_MODE_PIXEL),
        }

        self.advance_affine_scanline();
    }

    fn render_mode2_scanline(&mut self, line: usize, vram: &[u8], palette: &[u8], oam: &[u8]) {
        let backdrop = read_palette_color(palette, 0);
        let destination_start = line * SCREEN_WIDTH;

        self.render_object_scanline(line, vram, palette, oam);

        for x in 0..SCREEN_WIDTH {
            let background = self.sample_mode2_background_pixel(x, vram, palette);
            let object = self.object_line[x];
            let color = compose_background_and_object(background, object, backdrop);

            self.framebuffer[destination_start + x] = color;
        }
    }

    fn sample_mode2_background_pixel(
        &self,
        x: usize,
        vram: &[u8],
        palette: &[u8],
    ) -> Option<BackgroundPixel> {
        let bg2 = if self.display_control.bg2_enabled() {
            sample_affine_background(&self.bg2, x, vram, palette).map(|color| BackgroundPixel {
                color,
                priority: self.bg2.control.priority(),
                layer: 2,
            })
        } else {
            None
        };

        let bg3 = if self.display_control.bg3_enabled() {
            sample_affine_background(&self.bg3, x, vram, palette).map(|color| BackgroundPixel {
                color,
                priority: self.bg3.control.priority(),
                layer: 3,
            })
        } else {
            None
        };

        match (bg2, bg3) {
            (Some(bg2), Some(bg3)) => {
                if bg2.priority < bg3.priority
                    || (bg2.priority == bg3.priority && bg2.layer < bg3.layer)
                {
                    Some(bg2)
                } else {
                    Some(bg3)
                }
            }

            (Some(bg2), None) => Some(bg2),
            (None, Some(bg3)) => Some(bg3),
            (None, None) => None,
        }
    }

    fn render_mode3_scanline(&mut self, line: usize, vram: &[u8], palette: &[u8], oam: &[u8]) {
        let source_line_start = line * SCREEN_WIDTH * 2;
        let destination_line_start = line * SCREEN_WIDTH;
        let bg_priority = self.bg2.control.priority();

        self.render_object_scanline(line, vram, palette, oam);

        for x in 0..SCREEN_WIDTH {
            let source_offset = source_line_start + x * 2;
            let low = vram.get(source_offset).copied().unwrap_or(0);
            let high = vram.get(source_offset + 1).copied().unwrap_or(0);
            let background = BackgroundPixel {
                color: bgr555_to_rgba8888(u16::from_le_bytes([low, high])),
                priority: bg_priority,
                layer: 2,
            };

            self.framebuffer[destination_line_start + x] = compose_background_and_object(
                Some(background),
                self.object_line[x],
                background.color,
            );
        }
    }

    fn render_mode4_scanline(
        &mut self,
        line: usize,
        vram: &[u8],
        palette: &[u8],
        oam: &[u8],
    ) {
        /*
         * Mode 4:
         *
         * 240 × 160
         * 8-bit BG palette indices
         * page 0 at VRAM 0x0000
         * page 1 at VRAM 0xA000
         *
         * Palette index zero is a normal visible bitmap color.
         */
        let page_base = if self.display_control.page_selected() {
            0xA000
        } else {
            0
        };

        let source_line_start = page_base + line * SCREEN_WIDTH;
        let destination_line_start = line * SCREEN_WIDTH;
        let backdrop = read_palette_color(palette, 0);
        let bg_priority = self.bg2.control.priority();

        self.render_object_scanline(line, vram, palette, oam);

        for x in 0..SCREEN_WIDTH {
            let background = if self.display_control.bg2_enabled() {
                let palette_index = vram
                    .get(source_line_start + x)
                    .copied()
                    .unwrap_or(0);

                Some(BackgroundPixel {
                    color: read_palette_color(palette, palette_index),
                    priority: bg_priority,
                    layer: 2,
                })
            } else {
                None
            };

            self.framebuffer[destination_line_start + x] =
                compose_background_and_object(background, self.object_line[x], backdrop);
        }
    }

    fn render_object_scanline(
        &mut self,
        line: usize,
        vram: &[u8],
        palette: &[u8],
        oam: &[u8],
    ) {
        self.object_line.fill(None);

        if !self.display_control.obj_enabled() {
            return;
        }

        /*
         * Draw high OAM indices first so lower indices overwrite them,
         * matching GBA OBJ-to-OBJ ordering.
         */
        for object_index in (0..128usize).rev() {
            let Some(attributes) = read_object_attributes(oam, object_index) else {
                continue;
            };

            if attributes.disabled() || attributes.shape() == 3 || attributes.object_mode() >= 2 {
                continue;
            }

            let Some((texture_width, texture_height)) =
                object_dimensions(attributes.shape(), attributes.size())
            else {
                continue;
            };

            let display_width = if attributes.double_size() {
                texture_width * 2
            } else {
                texture_width
            };

            let display_height = if attributes.double_size() {
                texture_height * 2
            } else {
                texture_height
            };

            let object_y = attributes.y() as i32;
            let local_y = ((line as i32 - object_y) & 0xFF) as i32;

            if local_y >= display_height as i32 {
                continue;
            }

            let object_x = sign_extend_object_x(attributes.x());
            let start_x = object_x.max(0) as usize;
            let end_x = (object_x + display_width as i32).min(SCREEN_WIDTH as i32).max(0) as usize;

            for screen_x in start_x..end_x {
                let local_x = screen_x as i32 - object_x;

                let source = if attributes.affine() {
                    affine_object_source_coordinates(
                        attributes,
                        local_x,
                        local_y,
                        texture_width,
                        texture_height,
                        display_width,
                        display_height,
                        oam,
                    )
                } else {
                    regular_object_source_coordinates(
                        attributes,
                        local_x,
                        local_y,
                        texture_width,
                        texture_height,
                    )
                };

                let Some((source_x, source_y)) = source else {
                    continue;
                };

                let Some(color) = sample_object_texel(
                    self.display_control,
                    attributes,
                    source_x,
                    source_y,
                    texture_width,
                    vram,
                    palette,
                ) else {
                    continue;
                };

                self.object_line[screen_x] = Some(ObjectPixel {
                    color,
                    priority: attributes.priority(),
                    oam_index: object_index as u8,
                    semi_transparent: attributes.object_mode() == 1,
                });
            }
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
        self.object_line.fill(None);
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

    Some(read_palette_color(palette, palette_index))
}

fn compose_background_and_object(
    background: Option<BackgroundPixel>,
    object: Option<ObjectPixel>,
    backdrop: u32,
) -> u32 {
    match (background, object) {
        /*
         * When OBJ and BG priorities are equal, OBJ is above BG.
         */
        (Some(background), Some(object)) => {
            if object.priority <= background.priority {
                object.color
            } else {
                background.color
            }
        }

        (Some(background), None) => background.color,
        (None, Some(object)) => object.color,
        (None, None) => backdrop,
    }
}

fn read_object_attributes(oam: &[u8], object_index: usize) -> Option<ObjectAttributes> {
    let base = object_index.checked_mul(8)?;

    Some(ObjectAttributes {
        attr0: read_u16(oam, base)?,
        attr1: read_u16(oam, base + 2)?,
        attr2: read_u16(oam, base + 4)?,
    })
}

fn object_dimensions(shape: u8, size: u8) -> Option<(usize, usize)> {
    const DIMENSIONS: [[(usize, usize); 4]; 3] = [
        [(8, 8), (16, 16), (32, 32), (64, 64)],
        [(16, 8), (32, 8), (32, 16), (64, 32)],
        [(8, 16), (8, 32), (16, 32), (32, 64)],
    ];

    DIMENSIONS
        .get(shape as usize)
        .and_then(|sizes| sizes.get(size as usize))
        .copied()
}

fn sign_extend_object_x(value: u16) -> i32 {
    let value = (value & 0x01FF) as i32;

    if value >= 256 { value - 512 } else { value }
}

fn regular_object_source_coordinates(
    attributes: ObjectAttributes,
    local_x: i32,
    local_y: i32,
    texture_width: usize,
    texture_height: usize,
) -> Option<(usize, usize)> {
    let mut source_x = local_x as usize;
    let mut source_y = local_y as usize;

    if attributes.horizontal_flip() {
        source_x = texture_width - 1 - source_x;
    }

    if attributes.vertical_flip() {
        source_y = texture_height - 1 - source_y;
    }

    Some((source_x, source_y))
}

fn affine_object_source_coordinates(
    attributes: ObjectAttributes,
    local_x: i32,
    local_y: i32,
    texture_width: usize,
    texture_height: usize,
    display_width: usize,
    display_height: usize,
    oam: &[u8],
) -> Option<(usize, usize)> {
    let (pa, pb, pc, pd) = read_object_affine_matrix(oam, attributes.affine_parameter_index())?;

    let centered_x = local_x - display_width as i32 / 2;
    let centered_y = local_y - display_height as i32 / 2;

    let source_x =
        ((pa as i32 * centered_x + pb as i32 * centered_y) >> 8) + texture_width as i32 / 2;

    let source_y =
        ((pc as i32 * centered_x + pd as i32 * centered_y) >> 8) + texture_height as i32 / 2;

    if source_x < 0
        || source_y < 0
        || source_x >= texture_width as i32
        || source_y >= texture_height as i32
    {
        return None;
    }

    Some((source_x as usize, source_y as usize))
}

fn read_object_affine_matrix(oam: &[u8], parameter_index: usize) -> Option<(i16, i16, i16, i16)> {
    let base = parameter_index.checked_mul(32)?;

    Some((
        read_u16(oam, base + 6)? as i16,
        read_u16(oam, base + 14)? as i16,
        read_u16(oam, base + 22)? as i16,
        read_u16(oam, base + 30)? as i16,
    ))
}

fn sample_object_texel(
    display_control: DisplayControl,
    attributes: ObjectAttributes,
    source_x: usize,
    source_y: usize,
    texture_width: usize,
    vram: &[u8],
    palette: &[u8],
) -> Option<u32> {
    let color_8bpp = attributes.color_8bpp();
    let tile_units_per_tile = if color_8bpp { 2 } else { 1 };

    let mut base_tile = attributes.tile_number();

    /*
     * 8bpp OBJ tiles occupy two 32-byte tile-number units. Bit zero of the
     * base tile number is ignored.
     */
    if color_8bpp {
        base_tile &= !1;
    }

    let tile_x = source_x / 8;
    let tile_y = source_y / 8;

    let tile_unit = if display_control.obj_mapping_1d() {
        let row_stride = (texture_width / 8) * tile_units_per_tile;
        base_tile + tile_y * row_stride + tile_x * tile_units_per_tile
    } else {
        /*
         * 2D mapping always has a 32 tile-unit row boundary.
         */
        base_tile + tile_y * 32 + tile_x * tile_units_per_tile
    };

    let bitmap_mode = matches!(
        display_control.mode(),
        VideoMode::Mode3 | VideoMode::Mode4 | VideoMode::Mode5
    );

    /*
     * Modes 0-2 expose 32 KiB of OBJ tile VRAM at 0x10000.
     * Bitmap modes expose only the upper 16 KiB at 0x14000 and require
     * tile numbers 512-1023.
     */
    let object_vram_base = if bitmap_mode {
        if tile_unit < 512 {
            return None;
        }

        0x10000
    } else {
        0x10000
    };

    let tile_address = object_vram_base + tile_unit * 32;
    let pixel_x = source_x & 7;
    let pixel_y = source_y & 7;

    let palette_index = if color_8bpp {
        let address = tile_address + pixel_y * 8 + pixel_x;
        vram.get(address).copied().unwrap_or(0) as usize
    } else {
        let address = tile_address + pixel_y * 4 + pixel_x / 2;
        let packed = vram.get(address).copied().unwrap_or(0);

        let color = if pixel_x & 1 == 0 {
            packed & 0x0F
        } else {
            packed >> 4
        };

        if color == 0 {
            return None;
        }

        attributes.palette_bank() * 16 + color as usize
    };

    if palette_index == 0 {
        return None;
    }

    /*
     * OBJ palettes occupy entries 256-511 in palette RAM.
     */
    Some(read_palette_color_usize(palette, 256 + palette_index))
}

fn read_u16(memory: &[u8], offset: usize) -> Option<u16> {
    let low = *memory.get(offset)?;
    let high = *memory.get(offset + 1)?;

    Some(u16::from_le_bytes([low, high]))
}

fn read_palette_color(palette: &[u8], index: u8) -> u32 {
    read_palette_color_usize(palette, index as usize)
}

fn read_palette_color_usize(palette: &[u8], index: usize) -> u32 {
    let offset = index * 2;
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

        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

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
        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

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
        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

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

        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

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
        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn forced_blank_outputs_white() {
        let mut video = Video::new();
        video.write_display_control(3 | (1 << 7));

        let vram = vec![0u8; SCREEN_WIDTH * 2];
        let palette = vec![0u8; 0x400];

        video.render_scanline(0, &vram, &palette, &[0; 0x400]);

        assert!(
            video.framebuffer()[0..SCREEN_WIDTH]
                .iter()
                .all(|&pixel| pixel == Video::FORCED_BLANK_PIXEL)
        );
    }
    #[test]
    fn mode2_renders_regular_4bpp_object() {
        let mut video = Video::new();

        /*
         * Mode 2, OBJ enabled, 1D OBJ mapping.
         */
        video.write_display_control(2 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        /*
         * OBJ0: 8x8 at (0,0), regular, 4bpp, tile 0, priority 0,
         * palette bank 0.
         */
        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        /*
         * First OBJ pixel uses palette color 1.
         */
        vram[0x10000] = 0x01;
        palette[0x202..0x204].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn mode2_renders_8bpp_object_with_1d_mapping() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        /*
         * OBJ0: 16x16 square, 8bpp, tile number 0.
         */
        let attr0 = 1 << 13;
        let attr1 = 1 << 14;

        oam[0..2].copy_from_slice(&(attr0 as u16).to_le_bytes());
        oam[2..4].copy_from_slice(&(attr1 as u16).to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        /*
         * Pixel (8,0) is the first pixel of the second 8bpp tile. In 1D
         * mapping that tile begins two 32-byte units after tile zero.
         */
        vram[0x10000 + 64] = 2;
        palette[0x204..0x206].copy_from_slice(&0x03E0u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[8], 0xFF00_FF00);
    }

    #[test]
    fn regular_object_flip_is_applied() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        /*
         * 8x8 object with horizontal flip.
         */
        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&(1u16 << 12).to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        /*
         * Source x=7 contains color 1.
         */
        vram[0x10000 + 3] = 0x10;
        palette[0x202..0x204].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFF00_00FF);
    }

    #[test]
    fn affine_object_identity_matrix_renders() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        /*
         * OBJ0 affine 8x8, matrix group 0.
         */
        oam[0..2].copy_from_slice(&(1u16 << 8).to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        oam[6..8].copy_from_slice(&0x0100u16.to_le_bytes());
        oam[14..16].copy_from_slice(&0u16.to_le_bytes());
        oam[22..24].copy_from_slice(&0u16.to_le_bytes());
        oam[30..32].copy_from_slice(&0x0100u16.to_le_bytes());

        vram[0x10000] = 0x01;
        palette[0x202..0x204].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn lower_oam_index_wins_between_objects() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        /*
         * OBJ0 tile 0 and OBJ1 tile 1 overlap.
         */
        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        oam[8..10].copy_from_slice(&0u16.to_le_bytes());
        oam[10..12].copy_from_slice(&0u16.to_le_bytes());
        oam[12..14].copy_from_slice(&1u16.to_le_bytes());

        vram[0x10000] = 0x01;
        vram[0x10000 + 32] = 0x02;

        palette[0x202..0x204].copy_from_slice(&0x001Fu16.to_le_bytes());
        palette[0x204..0x206].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn object_wins_over_background_on_equal_priority() {
        let mut video = Video::new();
        video.write_display_control(2 | (1 << 4) | (1 << 10) | (1 << 12));

        {
            let bg2 = video.affine_background_mut(2);
            bg2.write_control(1 << 8);
            bg2.write_pa(0x0100);
            bg2.write_pd(0x0100);
        }

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        vram[0x800] = 1;
        vram[64] = 1;
        palette[2..4].copy_from_slice(&0x03E0u16.to_le_bytes());

        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        vram[0x10000] = 0x02;
        palette[0x204..0x206].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.begin_frame();
        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }
    #[test]
    fn mode4_renders_page_zero_with_bg_palette() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 10));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let oam = vec![0u8; 0x400];

        vram[0] = 1;
        vram[1] = 2;
        palette[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());
        palette[4..6].copy_from_slice(&0x03E0u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
        assert_eq!(video.framebuffer()[1], 0xFF00_FF00);
    }

    #[test]
    fn mode4_selects_page_one() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 4) | (1 << 10));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let oam = vec![0u8; 0x400];

        vram[0] = 1;
        vram[0xA000] = 2;
        palette[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());
        palette[4..6].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFF00_00FF);
    }

    #[test]
    fn mode4_palette_zero_is_visible() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 10));

        let vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let oam = vec![0u8; 0x400];

        palette[0..2].copy_from_slice(&0x03E0u16.to_le_bytes());
        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFF00_FF00);
    }

    #[test]
    fn mode4_uses_correct_last_pixel_address() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 10));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let oam = vec![0u8; 0x400];

        let line = SCREEN_HEIGHT - 1;
        let x = SCREEN_WIDTH - 1;
        vram[line * SCREEN_WIDTH + x] = 1;
        palette[2..4].copy_from_slice(&0x7FFFu16.to_le_bytes());

        video.render_scanline(line as u16, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[line * SCREEN_WIDTH + x], 0xFFFF_FFFF);
    }

    #[test]
    fn mode4_composes_obj_using_bg2_priority() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 4) | (1 << 10) | (1 << 12));
        video.affine_background_mut(2).write_control(2);

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        vram[0] = 1;
        palette[2..4].copy_from_slice(&0x03E0u16.to_le_bytes());

        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&(512u16 | (1 << 10)).to_le_bytes());

        vram[0x14000] = 0x02;
        palette[0x204..0x206].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
    }

    #[test]
    fn mode4_rejects_obj_tiles_below_512() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 4) | (1 << 10) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        vram[0] = 1;
        palette[2..4].copy_from_slice(&0x03E0u16.to_le_bytes());

        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&0u16.to_le_bytes());

        vram[0x10000] = 0x02;
        palette[0x204..0x206].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFF00_FF00);
    }

    #[test]
    fn mode4_without_bg2_uses_backdrop_and_still_renders_obj() {
        let mut video = Video::new();
        video.write_display_control(4 | (1 << 4) | (1 << 12));

        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];
        let mut oam = vec![0u8; 0x400];

        palette[0..2].copy_from_slice(&0x7C00u16.to_le_bytes());

        oam[0..2].copy_from_slice(&0u16.to_le_bytes());
        oam[2..4].copy_from_slice(&0u16.to_le_bytes());
        oam[4..6].copy_from_slice(&512u16.to_le_bytes());

        vram[0x14000] = 0x01;
        palette[0x202..0x204].copy_from_slice(&0x001Fu16.to_le_bytes());

        video.render_scanline(0, &vram, &palette, &oam);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000);
        assert_eq!(video.framebuffer()[8], 0xFF00_00FF);
    }

}
