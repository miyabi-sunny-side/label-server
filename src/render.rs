//! Text to 1-bit label bitmap.
//!
//! The sizing and placement rules follow `render_text()` of ptouch-print,
//! Copyright (C) 2015-2026 Dominic Radermacher <dominic@familie-radermacher.ch>,
//! licensed under the GNU General Public License version 3.

// Pixel arithmetic: glyph metrics are f32, bitmap coordinates are usize.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::fmt;

use ab_glyph::{Font, Glyph, PxScale, ScaleFont, point};

/// Row-major 1-bit image: `true` is ink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pixels: Vec<bool>,
}

impl Bitmap {
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![false; width * height],
        }
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.pixels[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize) {
        self.pixels[y * self.width + x] = true;
    }
}

#[derive(Debug)]
pub enum RenderError {
    NoLines,
    TooSmall { print_height: usize, lines: usize },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLines => write!(f, "nothing to render"),
            Self::TooSmall {
                print_height,
                lines,
            } => write!(f, "{lines} lines do not fit into {print_height}px of tape"),
        }
    }
}

impl std::error::Error for RenderError {}

const MIN_SCALE: u32 = 4;
/// Coverage above which an anti-aliased sample becomes ink.
const INK_THRESHOLD: f32 = 0.5;

struct Line {
    glyphs: Vec<Glyph>,
    /// Glyph bounding box: min x, max x, min y (negative = above baseline), max y.
    bounds: (f32, f32, f32, f32),
}

/// Lays out `text` at `scale` on a baseline at the origin.
fn layout<F: Font>(font: &F, text: &str, scale: PxScale) -> Line {
    let scaled = font.as_scaled(scale);
    let mut glyphs = Vec::new();
    let mut x = 0.0;
    let mut previous: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(prev) = previous {
            x += scaled.kern(prev, id);
        }
        glyphs.push(id.with_scale_and_position(scale, point(x, 0.0)));
        x += scaled.h_advance(id);
        previous = Some(id);
    }
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for glyph in &glyphs {
        if let Some(outlined) = font.outline_glyph(glyph.clone()) {
            let b = outlined.px_bounds();
            bounds = Some(match bounds {
                None => (b.min.x, b.max.x, b.min.y, b.max.y),
                Some(acc) => (
                    acc.0.min(b.min.x),
                    acc.1.max(b.max.x),
                    acc.2.min(b.min.y),
                    acc.3.max(b.max.y),
                ),
            });
        }
    }
    // no outlines at all (blank line): zero-height box as wide as the advance
    let bounds = bounds.unwrap_or((0.0, x.max(0.0), 0.0, 0.0));
    Line { glyphs, bounds }
}

fn ink_height(bounds: (f32, f32, f32, f32)) -> usize {
    (bounds.3 - bounds.2).ceil().max(0.0) as usize
}

/// Largest integer scale whose ink height fits `want_px` (ptouch-print's
/// `find_fontsize`), or `None` when even the smallest does not fit.
fn find_scale<F: Font>(font: &F, text: &str, want_px: usize) -> Option<u32> {
    let mut best = None;
    let mut scale = MIN_SCALE;
    loop {
        let line = layout(font, text, PxScale::from(scale as f32));
        if ink_height(line.bounds) > want_px {
            break;
        }
        best = Some(scale);
        scale += 1;
        if scale > 512 {
            break;
        }
    }
    best
}

/// Renders `lines` stacked top to bottom into a bitmap `print_height` px
/// tall, using the largest font size at which every line fits its slot.
///
/// # Errors
/// Returns [`RenderError::NoLines`] for an empty slice and
/// [`RenderError::TooSmall`] when even the smallest font does not fit.
pub fn render_lines<F: Font>(
    font: &F,
    lines: &[&str],
    print_height: usize,
) -> Result<Bitmap, RenderError> {
    if lines.is_empty() {
        return Err(RenderError::NoLines);
    }
    let slot = print_height / lines.len();
    let scale = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| find_scale(font, line, slot))
        .try_fold(u32::MAX, |acc, s| s.map(|s| acc.min(s)))
        .ok_or(RenderError::TooSmall {
            print_height,
            lines: lines.len(),
        })?;
    let scale = if scale == u32::MAX { MIN_SCALE } else { scale };
    let px = PxScale::from(scale as f32);
    let laid_out: Vec<Line> = lines.iter().map(|line| layout(font, line, px)).collect();

    let width = laid_out
        .iter()
        .map(|line| (line.bounds.1 - line.bounds.0).ceil().max(0.0) as usize)
        .max()
        .unwrap_or(0)
        .max(1);
    let max_height = laid_out
        .iter()
        .map(|line| ink_height(line.bounds))
        .max()
        .unwrap_or(0);
    if max_height * lines.len() > print_height {
        return Err(RenderError::TooSmall {
            print_height,
            lines: lines.len(),
        });
    }
    let unused = print_height - max_height * lines.len();

    let mut bitmap = Bitmap::new(width, print_height);
    for (i, line) in laid_out.iter().enumerate() {
        // Baseline sits so that the tallest ink of any line ends at the
        // bottom of this line's ink slot, centred in the remaining slack.
        let top = i * slot + (unused / lines.len()) / 2;
        let baseline = top as f32 + max_height as f32 - (line.bounds.3.max(0.0));
        let shift_x = -line.bounds.0;
        for glyph in &line.glyphs {
            let Some(outlined) = font.outline_glyph(glyph.clone()) else {
                continue;
            };
            let b = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                if coverage < INK_THRESHOLD {
                    return;
                }
                let x = b.min.x + gx as f32 + shift_x;
                let y = b.min.y + gy as f32 + baseline;
                if x < 0.0 || y < 0.0 {
                    return;
                }
                let (x, y) = (x as usize, y as usize);
                if x < bitmap.width && y < bitmap.height {
                    bitmap.set(x, y);
                }
            });
        }
    }
    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use ab_glyph::FontVec;

    use super::{Bitmap, render_lines};

    const FONT: &str = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc";

    fn font() -> FontVec {
        FontVec::try_from_vec_and_index(std::fs::read(FONT).unwrap(), 0).unwrap()
    }

    fn ink_rows(bitmap: &Bitmap) -> Vec<usize> {
        (0..bitmap.height)
            .filter(|&y| (0..bitmap.width).any(|x| bitmap.get(x, y)))
            .collect()
    }

    #[test]
    fn a_single_line_fills_the_tape_height() {
        let bitmap = render_lines(&font(), &["abc"], 76).unwrap();

        assert_eq!(bitmap.height, 76);
        assert!(bitmap.width > 0);
        let rows = ink_rows(&bitmap);
        assert!(!rows.is_empty());
        // the glyphs are large: ink spans more than half of the tape
        assert!(rows.last().unwrap() - rows.first().unwrap() > 38);
    }

    #[test]
    fn two_lines_each_stay_inside_their_half() {
        let bitmap = render_lines(&font(), &["abc", "abc"], 76).unwrap();

        assert_eq!(bitmap.height, 76);
        let rows = ink_rows(&bitmap);
        assert!(rows.iter().any(|&y| y < 38));
        assert!(rows.iter().any(|&y| y >= 38));
        // the second line is the first one shifted down by one slot
        for y in 0..38 {
            for x in 0..bitmap.width {
                assert_eq!(bitmap.get(x, y), bitmap.get(x, y + 38), "at ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_smaller_print_height_yields_a_shorter_label() {
        let full = render_lines(&font(), &["Gridfinity"], 76).unwrap();
        let inset = render_lines(&font(), &["Gridfinity"], 68).unwrap();

        assert_eq!(inset.height, 68);
        assert!(inset.width < full.width, "{} < {}", inset.width, full.width);
        assert!(!ink_rows(&inset).is_empty());
    }

    #[test]
    fn japanese_text_produces_ink() {
        let bitmap = render_lines(&font(), &["ラベル"], 128).unwrap();
        assert!(!ink_rows(&bitmap).is_empty());
    }

    #[test]
    fn blank_lines_keep_their_slot() {
        let bitmap = render_lines(&font(), &["abc", "", "abc"], 128).unwrap();
        let rows = ink_rows(&bitmap);
        assert!(rows.iter().any(|&y| y < 42));
        assert!(!rows.iter().any(|&y| (45..83).contains(&y)));
        assert!(rows.iter().any(|&y| y > 85));
    }
}
