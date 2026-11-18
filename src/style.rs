//! Theming.
//!
//! An [`EditorStyle`] combines a colour [`Palette`] and [`Typography`].
//! Use a preset ([`EditorStyle::light`], [`EditorStyle::dark`]), derive
//! one from a Frust theme ([`FromTheme::from_theme`]) or change
//! individual fields.

use crate::geometry::color_from_frust;
use frust::Theme;
use markdown_editor_core::style::{luminance, with_alpha};

pub use markdown_editor_core::style::{EditorStyle, Palette, Typography};

/// Derives a style from a Frust theme.
pub trait FromTheme {
    /// The style of `theme` (its active colour scheme), so the editor matches
    /// the rest of the application.
    fn from_theme(theme: &Theme) -> Self;
}

impl FromTheme for EditorStyle {
    fn from_theme(theme: &Theme) -> Self {
        let scheme = theme.scheme();
        let background = color_from_frust(scheme.surface);
        let text = color_from_frust(scheme.on_surface);
        let primary = color_from_frust(scheme.primary);
        let dark = luminance(background) < 0.5;
        let mut style = if dark { Self::dark() } else { Self::light() };
        let palette = &mut style.palette;
        palette.page = background;
        palette.background = color_from_frust(scheme.surface_container_high);
        palette.ink = text;
        palette.caret = text;
        palette.code = text;
        palette.marker = text;
        palette.toolbar_text = text;
        palette.toolbar_background = color_from_frust(scheme.surface_container);
        palette.toolbar_border = color_from_frust(scheme.outline_variant);
        palette.muted = color_from_frust(scheme.on_surface_variant);
        palette.link = primary;
        palette.accent = primary;
        palette.heading = primary;
        palette.selection = with_alpha(primary, 0.30);
        palette.toolbar_active = with_alpha(primary, 0.20);
        palette.toolbar_hover = with_alpha(text, 0.08);
        style
    }
}
