//! The bundled fonts.
//!
//! The same font files are used for measuring (by the core) and for drawing:
//! the editor draws glyph runs itself, placing every glyph at the position
//! the layout measured. That keeps the caret, selection and mouse
//! hit-testing exact, on the desktop and in the browser (where no system
//! fonts exist).

use frust::authoring::scene::FontHandle;
use frust::peniko::{Blob, FontData};
use markdown_editor_core::fonts::Fonts;
use std::sync::{Arc, OnceLock};

pub(crate) use markdown_editor_core::fonts::Face;

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

/// The UI faces used by the toolbar and status bar.
pub const SANS: Face = Face::Sans;
pub const SANS_BOLD: Face = Face::SansBold;
pub const SANS_ITALIC: Face = Face::SansItalic;
pub const MONO: Face = Face::Mono;

/// The process-wide metrics of the bundled fonts. The first call registers
/// the fonts with Frust, so the link bar's text field (and the application)
/// can use the `DejaVu` families.
pub(crate) fn shared() -> &'static Arc<Fonts> {
    static SHARED: OnceLock<Arc<Fonts>> = OnceLock::new();
    SHARED.get_or_init(|| {
        for bytes in FONTS {
            frust::register_app_fonts(bytes.to_vec());
        }
        Arc::new(Fonts::new(FONTS))
    })
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
