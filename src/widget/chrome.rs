//! The bars around the page (toolbar, link bar, status bar): a small item
//! model, a row layout that can wrap, and drawing in the editor palette.
//!
//! The bars are drawn by the editor widget itself, with the bundled fonts,
//! so they look the same everywhere and clicking them does not take the
//! keyboard focus from the page.

use super::Action;
use crate::draw::{Frame, Path, Stroke, Text};
use crate::fonts::{self, Face};
use crate::geometry::{Color, Point, Rectangle, Size};
use crate::style::Palette;

/// Line height relative to the font size.
pub(crate) const LINE: f32 = 1.3;

/// Inner padding: vertical, horizontal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pad {
    pub y: f32,
    pub x: f32,
}

impl Pad {
    pub(crate) const fn new(y: f32, x: f32) -> Pad {
        Pad { y, x }
    }
}

/// A push button.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Button {
    pub label: String,
    pub face: Face,
    pub size: f32,
    pub pad: Pad,
    /// Marks an active format or mode.
    pub on: bool,
    /// `None` = disabled.
    pub action: Option<Action>,
}

/// A drop-down list (the code language of the toolbar).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Dropdown {
    pub current: String,
    pub options: Vec<(String, Action)>,
    pub width: f32,
    pub size: f32,
    pub pad: Pad,
}

impl Dropdown {
    pub(crate) fn option_height(&self) -> f32 {
        self.size * LINE + 2.0 * self.pad.y
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    Button(Button),
    Label {
        text: String,
        size: f32,
        color: Color,
    },
    /// A thin vertical "|".
    Separator,
    /// Takes the remaining width (bars that do not wrap).
    Fill,
    Dropdown(Dropdown),
    /// Room for the link bar's text field (a Frust widget).
    Input {
        width: f32,
    },
    /// Items kept together with their own spacing.
    Row {
        items: Vec<Item>,
        spacing: f32,
    },
}

/// One bar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bar {
    pub items: Vec<Item>,
    pub padding: Pad,
    pub spacing: f32,
    /// Continue on the next line instead of overflowing.
    pub wrap: bool,
    pub background: Color,
    pub border: Option<Color>,
}

/// A laid out item. Rows are flattened; their parts are slots too.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Slot {
    pub rect: Rectangle,
    pub item: Item,
}

impl Slot {
    /// Whether the slot reacts to the pointer.
    pub(crate) fn interactive(&self) -> bool {
        match &self.item {
            Item::Button(button) => button.action.is_some(),
            Item::Dropdown(_) => true,
            _ => false,
        }
    }
}

pub(crate) fn text_width(text: &str, face: Face, size: f32) -> f32 {
    fonts::shared().width(face, text, size)
}

const SEPARATOR_SIZE: f32 = 16.0;

/// Natural size of an item (a fill is empty).
fn measure(item: &Item, input_height: f32) -> Size {
    match item {
        Item::Button(button) => Size::new(
            text_width(&button.label, button.face, button.size) + 2.0 * button.pad.x,
            button.size * LINE + 2.0 * button.pad.y,
        ),
        Item::Label { text, size, .. } => {
            Size::new(text_width(text, fonts::SANS, *size), size * LINE)
        }
        Item::Separator => Size::new(
            text_width("|", fonts::SANS, SEPARATOR_SIZE),
            SEPARATOR_SIZE * LINE,
        ),
        Item::Fill => Size::ZERO,
        Item::Dropdown(dropdown) => Size::new(dropdown.width, dropdown.option_height()),
        Item::Input { width } => Size::new(*width, input_height),
        Item::Row { items, spacing } => {
            let sizes: Vec<Size> = items.iter().map(|i| measure(i, input_height)).collect();
            let width = sizes.iter().map(|s| s.width).sum::<f32>()
                + spacing * items.len().saturating_sub(1) as f32;
            let height = sizes.iter().map(|s| s.height).fold(0.0, f32::max);
            Size::new(width, height)
        }
    }
}

/// Places `item` (and the parts of a row) with its top-left at `at`,
/// vertically centred in a line of `line_height`.
fn place(
    item: &Item,
    at: Point,
    width: f32,
    line_height: f32,
    input_height: f32,
    out: &mut Vec<Slot>,
) {
    let size = measure(item, input_height);
    let y = at.y + (line_height - size.height) / 2.0;
    match item {
        Item::Row { items, spacing } => {
            let mut x = at.x;
            for part in items {
                let part_size = measure(part, input_height);
                place(
                    part,
                    Point::new(x, at.y),
                    part_size.width,
                    line_height,
                    input_height,
                    out,
                );
                x += part_size.width + spacing;
            }
        }
        _ => out.push(Slot {
            rect: Rectangle::new(Point::new(at.x, y), Size::new(width, size.height)),
            item: item.clone(),
        }),
    }
}

/// The items of `bar` split into lines (indices), for an inner width.
fn split(bar: &Bar, sizes: &[Size], inner: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut x = 0.0;
    for (index, size) in sizes.iter().enumerate() {
        let line = lines.last_mut().expect("at least one line");
        let needed = if line.is_empty() {
            size.width
        } else {
            x + bar.spacing + size.width
        };
        if bar.wrap && !line.is_empty() && needed > inner {
            lines.push(vec![index]);
            x = size.width;
        } else {
            line.push(index);
            x = needed;
        }
    }
    lines
}

/// How many lines `bar` takes at the given width.
pub(crate) fn line_count(bar: &Bar, width: f32, input_height: f32) -> usize {
    let inner = (width - 2.0 * bar.padding.x).max(0.0);
    let sizes: Vec<Size> = bar.items.iter().map(|i| measure(i, input_height)).collect();
    split(bar, &sizes, inner).len()
}

/// Width `bar` needs for all its items on one line, padding included.
pub(crate) fn natural_width(bar: &Bar, input_height: f32) -> f32 {
    let items: f32 = bar
        .items
        .iter()
        .map(|i| measure(i, input_height).width)
        .sum();
    items + bar.spacing * bar.items.len().saturating_sub(1) as f32 + 2.0 * bar.padding.x
}

/// `bar` without the labels it has no room for: a bar that does not wrap
/// drops the labels before its first fill, the last one first, until it
/// fits `width`.
pub(crate) fn fit(bar: &Bar, width: f32, input_height: f32) -> Bar {
    let mut bar = bar.clone();
    if bar.wrap {
        return bar;
    }
    while natural_width(&bar, input_height) > width {
        let fill = bar
            .items
            .iter()
            .position(|item| matches!(item, Item::Fill))
            .unwrap_or(bar.items.len());
        let Some(last) = bar.items[..fill]
            .iter()
            .rposition(|item| matches!(item, Item::Label { .. }))
        else {
            break;
        };
        bar.items.remove(last);
    }
    bar
}

/// Lays out `bar` at `origin` with the given width; returns its height and
/// the slots in absolute coordinates.
pub(crate) fn layout(bar: &Bar, origin: Point, width: f32, input_height: f32) -> (f32, Vec<Slot>) {
    let inner = (width - 2.0 * bar.padding.x).max(0.0);
    let sizes: Vec<Size> = bar.items.iter().map(|i| measure(i, input_height)).collect();
    let lines = split(bar, &sizes, inner);

    let mut slots = Vec::new();
    let mut y = origin.y + bar.padding.y;
    for line in &lines {
        let line_height = line.iter().map(|&i| sizes[i].height).fold(0.0, f32::max);
        let used = line.iter().map(|&i| sizes[i].width).sum::<f32>()
            + bar.spacing * line.len().saturating_sub(1) as f32;
        let fills = line
            .iter()
            .filter(|&&i| matches!(bar.items[i], Item::Fill))
            .count();
        let extra = if fills > 0 && !bar.wrap {
            (inner - used).max(0.0) / fills as f32
        } else {
            0.0
        };
        let mut x = origin.x + bar.padding.x;
        for &i in line {
            let item = &bar.items[i];
            let width = match item {
                Item::Fill => extra,
                _ => sizes[i].width,
            };
            if !matches!(item, Item::Fill) {
                place(
                    item,
                    Point::new(x, y),
                    width,
                    line_height,
                    input_height,
                    &mut slots,
                );
            }
            x += width + bar.spacing;
        }
        y += line_height + bar.spacing;
    }
    let content = (y - bar.spacing) - (origin.y + bar.padding.y);
    (content.max(0.0) + 2.0 * bar.padding.y, slots)
}

/// Pointer state of a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Idle,
    Hovered,
    Pressed,
}

pub(crate) fn draw_label(
    frame: &mut Frame,
    text: &str,
    face: Face,
    size: f32,
    at: Point,
    color: Color,
) {
    frame.fill_text(Text {
        content: text.to_string(),
        position: at,
        color,
        size,
        line_height: Some(size * LINE),
        font: face,
        ..Text::default()
    });
}

pub(crate) fn rounded(
    frame: &mut Frame,
    rect: Rectangle,
    radius: f32,
    fill: Option<Color>,
    border: Option<Color>,
) {
    let path = Path::rounded_rectangle(rect.position(), rect.size(), radius);
    if let Some(fill) = fill {
        frame.fill(&path, fill);
    }
    if let Some(border) = border {
        let inset = Rectangle::new(
            Point::new(rect.x + 0.5, rect.y + 0.5),
            Size::new(rect.width - 1.0, rect.height - 1.0),
        );
        let outline = Path::rounded_rectangle(inset.position(), inset.size(), radius);
        frame.stroke(
            &outline,
            Stroke::default().with_width(1.0).with_color(border),
        );
    }
}

/// Draws a bar; `status(i)` gives the pointer state of slot `i`.
pub(crate) fn draw(
    frame: &mut Frame,
    bar: &Bar,
    rect: Rectangle,
    slots: &[Slot],
    palette: &Palette,
    status: impl Fn(usize) -> Status,
) {
    frame.fill_rectangle(rect.position(), rect.size(), bar.background);
    if let Some(border) = bar.border {
        frame.stroke_rectangle(
            Point::new(rect.x + 0.5, rect.y + 0.5),
            Size::new(rect.width - 1.0, rect.height - 1.0),
            Stroke::default().with_width(1.0).with_color(border),
        );
    }

    for (index, slot) in slots.iter().enumerate() {
        let r = slot.rect;
        match &slot.item {
            Item::Button(button) => {
                let enabled = button.action.is_some();
                let state = if enabled { status(index) } else { Status::Idle };
                let background = if !enabled {
                    None
                } else if button.on {
                    Some(palette.toolbar_active)
                } else if state != Status::Idle {
                    Some(palette.toolbar_hover)
                } else {
                    None
                };
                let border = button.on.then_some(palette.accent);
                rounded(frame, r, 4.0, background, border);
                let color = if !enabled {
                    palette.toolbar_text.scale_alpha(0.4)
                } else if button.on {
                    palette.accent
                } else {
                    palette.toolbar_text
                };
                draw_label(
                    frame,
                    &button.label,
                    button.face,
                    button.size,
                    Point::new(r.x + button.pad.x, r.y + button.pad.y),
                    color,
                );
            }
            Item::Label { text, size, color } => {
                draw_label(frame, text, fonts::SANS, *size, r.position(), *color);
            }
            Item::Separator => {
                draw_label(
                    frame,
                    "|",
                    fonts::SANS,
                    SEPARATOR_SIZE,
                    r.position(),
                    palette.toolbar_border,
                );
            }
            Item::Dropdown(dropdown) => {
                let border = if status(index) == Status::Idle {
                    palette.toolbar_border
                } else {
                    palette.accent
                };
                rounded(frame, r, 3.0, Some(palette.page), Some(border));
                let arrow = "\u{25BE}";
                let arrow_width = text_width(arrow, fonts::SANS, dropdown.size);
                frame.with_clip(
                    Rectangle::new(
                        r.position(),
                        Size::new((r.width - arrow_width - dropdown.pad.x).max(0.0), r.height),
                    ),
                    |frame| {
                        draw_label(
                            frame,
                            &dropdown.current,
                            fonts::SANS,
                            dropdown.size,
                            Point::new(r.x + dropdown.pad.x, r.y + dropdown.pad.y),
                            palette.toolbar_text,
                        );
                    },
                );
                draw_label(
                    frame,
                    arrow,
                    fonts::SANS,
                    dropdown.size,
                    Point::new(
                        r.x + r.width - dropdown.pad.x - arrow_width,
                        r.y + dropdown.pad.y,
                    ),
                    palette.toolbar_text,
                );
            }
            Item::Input { .. } | Item::Fill | Item::Row { .. } => {}
        }
    }
}

/// An open drop-down list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Menu {
    pub area: Rectangle,
    pub first: usize,
    pub visible: usize,
}

/// Where the options of `dropdown` (shown under `anchor`) go, within
/// `bounds`.
pub(crate) fn menu(
    dropdown: &Dropdown,
    anchor: Rectangle,
    bounds: Rectangle,
    first: usize,
) -> Menu {
    let height = dropdown.option_height();
    let below = bounds.y + bounds.height - (anchor.y + anchor.height) - 4.0;
    let fits = ((below / height).floor() as usize).max(3);
    let visible = dropdown.options.len().min(fits).max(1);
    let first = first.min(dropdown.options.len().saturating_sub(visible));
    Menu {
        area: Rectangle::new(
            Point::new(anchor.x, anchor.y + anchor.height),
            Size::new(anchor.width, visible as f32 * height),
        ),
        first,
        visible,
    }
}

impl Menu {
    /// The option under `point`.
    pub(crate) fn option_at(&self, dropdown: &Dropdown, point: Point) -> Option<usize> {
        if !self.area.contains(point) {
            return None;
        }
        let row = ((point.y - self.area.y) / dropdown.option_height()) as usize;
        let index = self.first + row;
        (index < dropdown.options.len()).then_some(index)
    }
}

pub(crate) fn draw_menu(
    frame: &mut Frame,
    dropdown: &Dropdown,
    menu: &Menu,
    hovered: Option<usize>,
    palette: &Palette,
) {
    let area = menu.area;
    frame.fill_rectangle(
        Point::new(area.x + 1.0, area.y + 2.0),
        area.size(),
        palette.page_shadow,
    );
    frame.fill_rectangle(area.position(), area.size(), palette.page);
    let height = dropdown.option_height();
    frame.with_clip(area, |frame| {
        for row in 0..menu.visible {
            let index = menu.first + row;
            let Some((label, _)) = dropdown.options.get(index) else {
                break;
            };
            let top = area.y + row as f32 * height;
            let selected = *label == dropdown.current;
            if hovered == Some(index) {
                frame.fill_rectangle(
                    Point::new(area.x, top),
                    Size::new(area.width, height),
                    palette.toolbar_active,
                );
            }
            draw_label(
                frame,
                label,
                if selected {
                    fonts::SANS_BOLD
                } else {
                    fonts::SANS
                },
                dropdown.size,
                Point::new(area.x + dropdown.pad.x, top + dropdown.pad.y),
                palette.toolbar_text,
            );
        }
    });
    frame.stroke_rectangle(
        Point::new(area.x + 0.5, area.y + 0.5),
        Size::new(area.width - 1.0, area.height - 1.0),
        Stroke::default()
            .with_width(1.0)
            .with_color(palette.toolbar_border),
    );
    // A thin scroll indicator when not all options fit.
    let total = dropdown.options.len();
    if total > menu.visible {
        let track = area.height - 4.0;
        let thumb = (track * menu.visible as f32 / total as f32).max(12.0);
        let max_first = (total - menu.visible) as f32;
        let y = area.y + 2.0 + (track - thumb) * menu.first as f32 / max_first;
        frame.fill_rectangle(
            Point::new(area.x + area.width - 5.0, y),
            Size::new(3.0, thumb),
            palette.muted.scale_alpha(0.5),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(label: &str) -> Item {
        Item::Button(Button {
            label: label.into(),
            face: fonts::SANS,
            size: 13.0,
            pad: Pad::new(5.0, 9.0),
            on: false,
            action: None,
        })
    }

    fn bar(wrap: bool, items: Vec<Item>) -> Bar {
        Bar {
            items,
            padding: Pad::new(4.0, 8.0),
            spacing: 2.0,
            wrap,
            background: Color::WHITE,
            border: None,
        }
    }

    #[test]
    fn wrapping_bars_grow_in_height() {
        let items: Vec<Item> = (0..20).map(|i| button(&format!("Button {i}"))).collect();
        let (wide, slots) = layout(&bar(true, items.clone()), Point::ORIGIN, 5000.0, 0.0);
        assert_eq!(slots.len(), 20);
        let (narrow, slots) = layout(&bar(true, items), Point::ORIGIN, 300.0, 0.0);
        assert!(narrow > wide * 3.0, "{narrow} vs {wide}");
        assert!(
            slots
                .iter()
                .all(|slot| slot.rect.x + slot.rect.width <= 300.0)
        );
    }

    #[test]
    fn fills_push_items_to_the_end() {
        let items = vec![button("a"), Item::Fill, button("b")];
        let (_, slots) = layout(&bar(false, items), Point::ORIGIN, 400.0, 0.0);
        assert_eq!(slots.len(), 2);
        let last = &slots[1].rect;
        assert!((last.x + last.width - (400.0 - 8.0)).abs() < 0.01);
    }

    #[test]
    fn narrow_bars_drop_the_labels_before_the_fill() {
        let label = |text: &str| Item::Label {
            text: text.into(),
            size: 12.0,
            color: Color::WHITE,
        };
        let items = vec![
            label("Page 1 of 3"),
            label("120 words"),
            label("Tab width: 4"),
            Item::Fill,
            label("Modified"),
            button("+"),
        ];
        let full = bar(false, items);
        let width = natural_width(&full, 0.0);
        assert_eq!(fit(&full, width, 0.0), full);

        let narrow = fit(&full, width - 1.0, 0.0);
        assert_eq!(narrow.items.len(), 5);
        assert!(!narrow.items.contains(&label("Tab width: 4")));

        let tiny = fit(&full, 10.0, 0.0);
        assert_eq!(tiny.items, vec![Item::Fill, label("Modified"), button("+")]);

        // Wrapping bars keep everything.
        let wrapping = bar(true, full.items.clone());
        assert_eq!(fit(&wrapping, 10.0, 0.0), wrapping);
    }

    #[test]
    fn line_count_follows_the_width() {
        let items: Vec<Item> = (0..20).map(|i| button(&format!("Button {i}"))).collect();
        let wrapping = bar(true, items);
        let width = natural_width(&wrapping, 0.0);
        assert_eq!(line_count(&wrapping, width, 0.0), 1);
        assert!(line_count(&wrapping, width / 2.0, 0.0) >= 2);
        assert!(line_count(&wrapping, width / 4.0, 0.0) > 3);
    }

    #[test]
    fn rows_are_flattened() {
        let items = vec![Item::Row {
            items: vec![button("-"), button("100 %"), button("+")],
            spacing: 2.0,
        }];
        let (_, slots) = layout(&bar(false, items), Point::ORIGIN, 400.0, 0.0);
        assert_eq!(slots.len(), 3);
        assert!(slots[1].rect.x > slots[0].rect.x);
    }
}
