use std::collections::HashMap;

use fontdue::{Font, Metrics};

use crate::ui::FontId;

/// UV coordinates and layout metrics for a single rasterized glyph.
#[derive(Clone, Debug)]
pub struct GlyphEntry {
    /// Top-left UV in the atlas (0..1).
    pub uv_min: [f32; 2],
    /// Bottom-right UV in the atlas (0..1).
    pub uv_max: [f32; 2],
    /// Horizontal advance in pixels.
    pub advance_x: f32,
    /// Left bearing: pixels to add to the cursor before drawing.
    pub bearing_x: f32,
    /// Top bearing: pixels above the baseline where the glyph top sits.
    pub bearing_y: f32,
    pub width: u32,
    pub height: u32,
}

/// Single-page R8 glyph atlas with row-based packing.
///
/// The atlas grows in height (doubling) when a new row would overflow.
/// Width is fixed at construction time. Cached UV coordinates are rescaled
/// when the atlas grows so they remain valid.
pub struct GlyphAtlas {
    pub width: u32,
    pub height: u32,
    /// Raw R8 pixel data, row-major.
    pub data: Vec<u8>,
    entries: HashMap<(FontId, u32, char), GlyphEntry>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Set whenever `data` is written so the backend knows to re-upload.
    pub dirty: bool,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new(1024, 1024)
    }
}

impl GlyphAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height) as usize],
            entries: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            dirty: false,
        }
    }

    /// Return the cached entry for `(font_id, size_px, ch)`, rasterizing and
    /// packing the glyph into the atlas if it is not yet present.
    pub fn get_or_rasterize(
        &mut self,
        font_id: FontId,
        font: &Font,
        size_px: f32,
        ch: char,
    ) -> GlyphEntry {
        let key = (font_id, size_px.to_bits(), ch);
        if let Some(entry) = self.entries.get(&key) {
            return entry.clone();
        }
        let (metrics, bitmap) = font.rasterize(ch, size_px);
        let entry = self.pack(metrics, &bitmap);
        self.entries.insert(key, entry.clone());
        entry
    }

    /// Pack a rasterized glyph bitmap into the atlas and return its entry.
    fn pack(&mut self, metrics: Metrics, bitmap: &[u8]) -> GlyphEntry {
        let glyph_w = metrics.width as u32;
        let glyph_h = metrics.height as u32;

        // Zero-size glyphs (e.g. space) need no atlas space.
        if glyph_w == 0 || glyph_h == 0 {
            return GlyphEntry {
                uv_min: [0.0; 2],
                uv_max: [0.0; 2],
                advance_x: metrics.advance_width,
                bearing_x: metrics.xmin as f32,
                bearing_y: (metrics.ymin + metrics.height as i32) as f32,
                width: 0,
                height: 0,
            };
        }

        // Advance to next row if glyph doesn't fit horizontally.
        if self.cursor_x + glyph_w > self.width {
            self.cursor_y += self.row_height + 1;
            self.cursor_x = 0;
            self.row_height = 0;
        }

        // Grow atlas vertically if needed.
        if self.cursor_y + glyph_h > self.height {
            self.grow_height();
        }

        let x = self.cursor_x;
        let y = self.cursor_y;

        // Write bitmap rows into atlas.
        for row in 0..glyph_h {
            let dst = ((y + row) * self.width + x) as usize;
            let src = (row * glyph_w) as usize;
            self.data[dst..dst + glyph_w as usize]
                .copy_from_slice(&bitmap[src..src + glyph_w as usize]);
        }

        self.cursor_x += glyph_w + 1;
        if glyph_h > self.row_height {
            self.row_height = glyph_h;
        }
        self.dirty = true;

        let uv_min = [x as f32 / self.width as f32, y as f32 / self.height as f32];
        let uv_max = [
            (x + glyph_w) as f32 / self.width as f32,
            (y + glyph_h) as f32 / self.height as f32,
        ];

        GlyphEntry {
            uv_min,
            uv_max,
            advance_x: metrics.advance_width,
            bearing_x: metrics.xmin as f32,
            bearing_y: (metrics.ymin + metrics.height as i32) as f32,
            width: glyph_w,
            height: glyph_h,
        }
    }

    fn grow_height(&mut self) {
        let old_h = self.height as f32;
        let new_h = self.height * 2;
        let mut new_data = vec![0u8; (self.width * new_h) as usize];
        new_data[..self.data.len()].copy_from_slice(&self.data);
        self.data = new_data;
        let scale = old_h / new_h as f32;
        for entry in self.entries.values_mut() {
            entry.uv_min[1] *= scale;
            entry.uv_max[1] *= scale;
        }
        self.height = new_h;
        self.dirty = true;
    }

    /// Pack the R8 atlas data into a `Vec<u32>` suitable for a GPU storage
    /// buffer (`uint data[]` in GLSL, 4 bytes per element, little-endian).
    pub fn pack_as_u32(&self) -> Vec<u32> {
        let padded_len = (self.data.len() + 3) / 4;
        let mut out = vec![0u32; padded_len];
        for (i, &byte) in self.data.iter().enumerate() {
            out[i / 4] |= (byte as u32) << ((i % 4) * 8);
        }
        out
    }
}

/// Return the total advance width of a single-line text string in pixels.
pub fn measure_text_width(font: &Font, size_px: f32, text: &str) -> f32 {
    text.chars()
        .map(|c| font.metrics(c, size_px).advance_width)
        .sum()
}

/// Return `(width, height)` for a single line of `text`. MVP: no word-wrap;
/// the returned width is the sum of all glyph advances, clamped to `max_width`
/// if provided. Height is `size_px`.
pub fn layout_text(font: &Font, size_px: f32, text: &str, max_width: Option<f32>) -> (f32, f32) {
    let width = measure_text_width(font, size_px, text);
    let clamped = match max_width {
        Some(mw) => width.min(mw),
        None => width,
    };
    (clamped, size_px)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_metrics(w: usize, h: usize, advance: f32) -> Metrics {
        Metrics {
            xmin: 0,
            ymin: 0,
            width: w,
            height: h,
            advance_width: advance,
            advance_height: 0.0,
            bounds: fontdue::OutlineBounds {
                xmin: 0.0,
                ymin: 0.0,
                width: w as f32,
                height: h as f32,
            },
        }
    }

    #[test]
    fn atlas_row_packing() {
        let mut atlas = GlyphAtlas::new(64, 64);
        let bitmap_a = vec![255u8; 8 * 8];
        let entry_a = atlas.pack(fake_metrics(8, 8, 10.0), &bitmap_a);
        assert_eq!(entry_a.width, 8);
        assert_eq!(entry_a.height, 8);
        assert!((entry_a.uv_min[0] - 0.0).abs() < 0.001);
        assert!((entry_a.uv_min[1] - 0.0).abs() < 0.001);
        assert!((entry_a.uv_max[0] - 8.0 / 64.0).abs() < 0.001);
        assert!((entry_a.uv_max[1] - 8.0 / 64.0).abs() < 0.001);

        // Second glyph placed to the right.
        let bitmap_b = vec![128u8; 8 * 8];
        let entry_b = atlas.pack(fake_metrics(8, 8, 10.0), &bitmap_b);
        assert!(
            (entry_b.uv_min[0] - 9.0 / 64.0).abs() < 0.001,
            "b starts after a+padding"
        );

        // Verify pixel data was written.
        assert_eq!(atlas.data[0], 255);
        assert_eq!(atlas.data[9], 128);
    }

    #[test]
    fn atlas_grows_when_full() {
        // Atlas is only 16 tall; packing a glyph taller than that forces a grow.
        let mut atlas = GlyphAtlas::new(64, 16);
        let bitmap = vec![1u8; 8 * 8];
        atlas.pack(fake_metrics(8, 8, 10.0), &bitmap);
        // This second glyph would overflow the original 16-row atlas if it
        // goes to a second row; since cursor_y(=9) + 8 > 16, grow fires.
        let bitmap2 = vec![2u8; 8 * 8];
        atlas.pack(fake_metrics(64, 8, 10.0), &bitmap2); // forces wrap then grow
        assert_eq!(atlas.height, 32, "atlas should have doubled");
        assert!(atlas.dirty);
    }

    #[test]
    fn zero_size_glyph_returns_without_atlas_space() {
        let mut atlas = GlyphAtlas::new(64, 64);
        let entry = atlas.pack(fake_metrics(0, 0, 5.0), &[]);
        assert_eq!(entry.width, 0);
        assert_eq!(entry.height, 0);
        assert_eq!(atlas.cursor_x, 0, "cursor should not advance");
    }
}
