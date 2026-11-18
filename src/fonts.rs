//! Bundled fonts.
//!
//! The same font files are used for measuring (`ab_glyph`) and for drawing:
//! the editor draws glyph runs itself, placing every glyph at the position
//! the layout measured. That keeps the caret, selection and mouse
//! hit-testing exact, on the desktop and in the browser (where no system
//! fonts exist).

use ab_glyph::{Font as _, FontRef};
use frust::authoring::scene::{FontHandle, Glyph, GlyphRun};
use frust::peniko::{Blob, FontData};
use std::sync::{Arc, Once, OnceLock};

pub const SANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
pub const SANS_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
pub const SANS_ITALIC_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Oblique.ttf");
pub const SANS_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSans-BoldOblique.ttf");
pub const MONO_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");
pub const MONO_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono-Bold.ttf");

/// All bundled font files.
///
/// The editor draws with them directly and registers them with Frust
/// (`frust::register_app_fonts`) on first use, so the families
/// `DejaVu Sans` and `DejaVu Sans Mono` are also available to the rest of
/// the application.
pub const FONTS: [&[u8]; 6] = [
    SANS_REGULAR_BYTES,
    SANS_BOLD_BYTES,
    SANS_ITALIC_BYTES,
    SANS_BOLD_ITALIC_BYTES,
    MONO_REGULAR_BYTES,
    MONO_BOLD_BYTES,
];

/// Family name of the bundled sans-serif font.
pub const SANS_FAMILY: &str = "DejaVu Sans";
/// Family name of the bundled monospace font.
pub const MONO_FAMILY: &str = "DejaVu Sans Mono";

/// A concrete font face. The discriminant is the index into [`FONTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Face {
    #[default]
    Sans = 0,
    SansBold = 1,
    SansItalic = 2,
    SansBoldItalic = 3,
    Mono = 4,
    MonoBold = 5,
}

/// The UI faces used by the toolbar and status bar.
pub const SANS: Face = Face::Sans;
pub const SANS_BOLD: Face = Face::SansBold;
pub const SANS_ITALIC: Face = Face::SansItalic;
pub const MONO: Face = Face::Mono;

impl Face {
    pub fn select(bold: bool, italic: bool, mono: bool) -> Face {
        match (mono, bold, italic) {
            (true, true, _) => Face::MonoBold,
            (true, false, _) => Face::Mono,
            (false, true, true) => Face::SansBoldItalic,
            (false, true, false) => Face::SansBold,
            (false, false, true) => Face::SansItalic,
            (false, false, false) => Face::Sans,
        }
    }
}

/// Glyph metrics for all bundled faces.
pub struct Fonts {
    faces: Vec<FontRef<'static>>,
    units_per_em: Vec<f32>,
}

impl Fonts {
    pub fn load() -> Self {
        register_with_frust();
        let mut faces = Vec::with_capacity(FONTS.len());
        let mut units_per_em = Vec::with_capacity(FONTS.len());

        for bytes in FONTS {
            let face = FontRef::try_from_slice(bytes).expect("bundled font must be valid");
            units_per_em.push(face.units_per_em().unwrap_or(2048.0));
            faces.push(face);
        }

        Self {
            faces,
            units_per_em,
        }
    }

    /// Horizontal advance of `c` in logical pixels.
    pub fn advance(&self, face: Face, c: char, size: f32) -> f32 {
        let index = face as usize;
        let font = &self.faces[index];
        let glyph = font.glyph_id(c);
        font.h_advance_unscaled(glyph) * size / self.units_per_em[index]
    }

    /// Width of a whole string in logical pixels.
    pub fn width(&self, face: Face, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(face, c, size)).sum()
    }
}

/// The process-wide metrics, for drawing code that has no `Fonts` at hand.
pub(crate) fn shared() -> &'static Fonts {
    static FONTS_CELL: OnceLock<Fonts> = OnceLock::new();
    FONTS_CELL.get_or_init(Fonts::load)
}

/// Glyph id of `c` in `face`.
pub(crate) fn glyph_id(face: Face, c: char) -> u32 {
    shared().faces[face as usize].glyph_id(c).0 as u32
}

/// Distance from the top of a line box of `line_height` to the baseline
/// for text of `size`: the glyphs' ascent-descent box is centred in the line.
pub(crate) fn baseline_offset(face: Face, size: f32, line_height: f32) -> f32 {
    let fonts = shared();
    let index = face as usize;
    let font = &fonts.faces[index];
    let scale = size / fonts.units_per_em[index];
    let ascent = font.ascent_unscaled() * scale;
    let descent = font.descent_unscaled() * scale; // negative
    (line_height - (ascent - descent)) / 2.0 + ascent
}

/// Frust font handle of a face (the bundled bytes, shared).
pub(crate) fn handle(face: Face) -> FontHandle {
    static HANDLES: OnceLock<Vec<FontData>> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        FONTS
            .iter()
            .map(|bytes| {
                let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(*bytes);
                FontData::new(Blob::new(data), 0)
            })
            .collect()
    });
    FontHandle::new(handles[face as usize].clone())
}

/// Lays out `text` on one line starting at `x` with baseline `baseline`;
/// the glyph x positions follow the measured advances.
pub(crate) fn run(
    text: &str,
    face: Face,
    size: f32,
    x: f32,
    baseline: f32,
    color: crate::geometry::Color,
) -> GlyphRun {
    let fonts = shared();
    let mut glyphs = Vec::with_capacity(text.len());
    let mut pen = x;
    for c in text.chars() {
        if !c.is_whitespace() {
            glyphs.push(Glyph {
                id: glyph_id(face, c),
                x: pen,
                y: baseline,
            });
        }
        pen += fonts.advance(face, c, size);
    }
    GlyphRun {
        font: handle(face),
        font_size: size,
        brush: frust::peniko::Brush::Solid(color.to_frust()),
        transform: frust::kurbo::Affine::IDENTITY,
        glyphs,
    }
}

/// Registers the bundled fonts with Frust once, so the link bar's text
/// field (and the application) can use the `DejaVu` families.
fn register_with_frust() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for bytes in FONTS {
            frust::register_app_fonts(bytes.to_vec());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_follow_measured_advances() {
        let fonts = shared();
        let run = run(
            "ab c",
            Face::Sans,
            20.0,
            5.0,
            30.0,
            crate::geometry::Color::BLACK,
        );
        // The space has no glyph but advances the pen.
        assert_eq!(run.glyphs.len(), 3);
        assert_eq!(run.glyphs[0].x, 5.0);
        let a = fonts.advance(Face::Sans, 'a', 20.0);
        assert!((run.glyphs[1].x - (5.0 + a)).abs() < 1e-4);
        assert!(run.glyphs.iter().all(|glyph| glyph.y == 30.0));
    }

    #[test]
    fn baseline_is_inside_the_line() {
        let offset = baseline_offset(Face::Sans, 14.0, 22.0);
        assert!(offset > 11.0 && offset < 22.0, "{offset}");
    }
}
