//! The link dialog, opened with "Link…" in the context menu: a modal box
//! in the middle of the editor with the text to display and the address,
//! as in word processors. Enter applies, Escape cancels, Tab switches
//! between the fields.
//!
//! The two text fields are Frust widgets owned by the editor widget; this
//! module lays out and draws the rest.

use super::chrome::{self, Bar, Button, Item, Pad, Slot, Status, draw_label, rounded};
use super::{Action, Inner};
use crate::draw::Frame;
use crate::fonts;
use crate::geometry::{Color, Point, Rectangle, Size};
use crate::i18n::{self, Language};
use crate::style::Palette;

pub(crate) use markdown_editor_core::editor::LinkDialog;

/// Width of the dialog box.
const WIDTH: f32 = 440.0;
/// Inner padding of the box.
const PADDING: f32 = 18.0;
/// Space between the sections.
const SPACING: f32 = 14.0;
/// Space between a caption and its field.
const CAPTION_GAP: f32 = 4.0;
const TITLE_SIZE: f32 = 15.0;
const CAPTION_SIZE: f32 = 12.0;

/// What the widget draws, built from the state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DialogView {
    title: String,
    /// Caption of the "text to display" field, when there is one.
    text_caption: Option<String>,
    url_caption: String,
    buttons: Bar,
}

pub(crate) fn view(dialog: &LinkDialog, palette: &Palette, language: Language) -> DialogView {
    let tr = |id| i18n::text(language, id);
    let button = |label: String, inner: Inner, on: bool| {
        Item::Button(Button {
            label,
            face: fonts::SANS,
            size: 12.0,
            pad: Pad::new(5.0, 14.0),
            on,
            action: Some(Action(inner)),
        })
    };
    let mut items = Vec::new();
    if dialog.editing {
        items.push(button(tr("link-remove"), Inner::LinkRemove, false));
    }
    items.extend([
        Item::Fill,
        button(tr("link-ok"), Inner::LinkApply, true),
        button(tr("link-cancel"), Inner::LinkCancel, false),
    ]);
    DialogView {
        title: tr(if dialog.editing {
            "link-dialog-edit"
        } else {
            "link-dialog-insert"
        }),
        text_caption: dialog.text.is_some().then(|| tr("link-text")),
        url_caption: tr("link-address"),
        buttons: Bar {
            items,
            padding: Pad::new(0.0, 0.0),
            spacing: 8.0,
            wrap: false,
            background: palette.toolbar_background,
            border: None,
        },
    }
}

impl DialogView {
    pub(crate) fn has_text(&self) -> bool {
        self.text_caption.is_some()
    }
}

/// Width of the fields for a widget of width `area`.
pub(crate) fn field_width(area: f32) -> f32 {
    box_width(area) - 2.0 * PADDING
}

fn box_width(area: f32) -> f32 {
    WIDTH.min(area - 24.0).max(200.0)
}

/// The laid out dialog, in widget coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Layout {
    pub frame: Rectangle,
    title: Point,
    text_caption: Option<Point>,
    /// Top-left corners of the fields.
    pub text_field: Option<Point>,
    url_caption: Point,
    pub url_field: Point,
    pub buttons: (Rectangle, Vec<Slot>),
}

/// Lays the dialog out in the middle of `area`; `field_height` is the
/// height of the text fields.
pub(crate) fn layout(view: &DialogView, area: Size, field_height: f32) -> Layout {
    let width = box_width(area.width);
    let inner = width - 2.0 * PADDING;
    let (buttons_height, _) = chrome::layout(&view.buttons, Point::ORIGIN, inner, 0.0);
    let caption = CAPTION_SIZE * chrome::LINE;
    let field = caption + CAPTION_GAP + field_height;
    let fields = if view.has_text() { 2.0 } else { 1.0 };
    let height = 2.0 * PADDING
        + TITLE_SIZE * chrome::LINE
        + fields * (SPACING + field)
        + SPACING
        + buttons_height;

    let x = ((area.width - width) / 2.0).max(0.0).round();
    let y = ((area.height - height) / 2.0).max(0.0).round();
    let left = x + PADDING;
    let mut top = y + PADDING;

    let title = Point::new(left, top);
    top += TITLE_SIZE * chrome::LINE + SPACING;
    let (text_caption, text_field) = if view.has_text() {
        let at = Point::new(left, top);
        top += field + SPACING;
        (
            Some(at),
            Some(Point::new(left, at.y + caption + CAPTION_GAP)),
        )
    } else {
        (None, None)
    };
    let url_caption = Point::new(left, top);
    let url_field = Point::new(left, top + caption + CAPTION_GAP);
    top += field + SPACING;
    let (_, slots) = chrome::layout(&view.buttons, Point::new(left, top), inner, 0.0);

    Layout {
        frame: Rectangle::new(Point::new(x, y), Size::new(width, height)),
        title,
        text_caption,
        text_field,
        url_caption,
        url_field,
        buttons: (
            Rectangle::new(Point::new(left, top), Size::new(inner, buttons_height)),
            slots,
        ),
    }
}

/// Draws the dimmed backdrop over `area` and the box (the fields are
/// painted by the widget afterwards).
pub(crate) fn draw(
    frame: &mut Frame,
    view: &DialogView,
    layout: &Layout,
    area: Size,
    palette: &Palette,
    status: impl Fn(usize) -> Status,
) {
    frame.fill_rectangle(Point::ORIGIN, area, Color::BLACK.scale_alpha(0.2));

    let rect = layout.frame;
    for (grow, alpha) in [(10.0, 0.04), (6.0, 0.06), (3.0, 0.08)] {
        let shadow = Rectangle::new(
            Point::new(rect.x - grow, rect.y - grow + 6.0),
            Size::new(rect.width + 2.0 * grow, rect.height + 2.0 * grow),
        );
        rounded(
            frame,
            shadow,
            8.0 + grow,
            Some(Color::BLACK.scale_alpha(alpha)),
            None,
        );
    }
    rounded(
        frame,
        rect,
        8.0,
        Some(palette.toolbar_background),
        Some(palette.toolbar_border),
    );

    draw_label(
        frame,
        &view.title,
        fonts::SANS_BOLD,
        TITLE_SIZE,
        layout.title,
        palette.toolbar_text,
    );
    let captions = [
        (view.text_caption.as_deref(), layout.text_caption),
        (Some(view.url_caption.as_str()), Some(layout.url_caption)),
    ];
    for (caption, at) in captions {
        if let (Some(caption), Some(at)) = (caption, at) {
            draw_label(frame, caption, fonts::SANS, CAPTION_SIZE, at, palette.muted);
        }
    }

    let (bar_rect, slots) = &layout.buttons;
    chrome::draw(frame, &view.buttons, *bar_rect, slots, palette, status);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(text: Option<&str>, editing: bool) -> LinkDialog {
        LinkDialog {
            text: text.map(str::to_string),
            original: text.unwrap_or_default().to_string(),
            editing,
        }
    }

    #[test]
    fn the_dialog_is_centred_and_its_fields_inside() {
        let palette = crate::style::EditorStyle::default().palette;
        let area = Size::new(1000.0, 700.0);
        let view = view(&dialog(Some("hi"), false), &palette, Language::default());
        let layout = layout(&view, area, 30.0);
        let frame = layout.frame;
        assert!((frame.x + frame.width / 2.0 - 500.0).abs() <= 1.0);
        assert!((frame.y + frame.height / 2.0 - 350.0).abs() <= 1.0);
        let text = layout.text_field.expect("text field");
        assert!(text.y < layout.url_field.y);
        assert!(layout.url_field.y + 30.0 < layout.buttons.0.y);
        assert!(layout.buttons.0.y + layout.buttons.0.height <= frame.y + frame.height);
        // OK and Cancel, no "Remove link" for a new link.
        assert_eq!(layout.buttons.1.len(), 2);
    }

    #[test]
    fn editing_offers_remove_and_hides_the_text_it_cannot_retype() {
        let palette = crate::style::EditorStyle::default().palette;
        let view = view(&dialog(None, true), &palette, Language::default());
        assert!(!view.has_text());
        let layout = layout(&view, Size::new(1000.0, 700.0), 30.0);
        assert!(layout.text_field.is_none());
        assert_eq!(layout.buttons.1.len(), 3);
    }
}
