//! The context menu opened with the right mouse button.
//!
//! With a selection it offers clipboard and inline formatting; with the
//! caret in a table (no selection) it offers row, column and alignment
//! commands. The editor widget draws it above the page.

use super::chrome::{draw_label, rounded, text_width};
use super::{Action, Command, EditorState, Inner, Mode};
use crate::doc::ColumnAlign;
use crate::draw::{Frame, Path, Stroke};
use crate::fonts;
use crate::geometry::{Color, Point, Rectangle, Size};
use crate::i18n;
use crate::style::Palette;

/// Smallest width of the menu.
const MIN_WIDTH: f32 = 220.0;
/// Height of one entry.
const ITEM: f32 = 26.0;
/// Height of one entry opened with a finger.
const TOUCH_ITEM: f32 = 40.0;
/// Height of a separator (line and space around it).
const SEPARATOR: f32 = 9.0;
/// Inner padding of the menu.
const PADDING: f32 = 4.0;
/// Horizontal padding of an entry.
const ITEM_PAD: f32 = 8.0;
/// Room for the check mark.
const CHECK: f32 = 18.0;
/// Space between a label and its shortcut.
const GAP: f32 = 24.0;
/// Font size of the labels.
const SIZE: f32 = 13.0;
/// Font size of the shortcuts.
const HINT_SIZE: f32 = 12.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    Item {
        label: String,
        hint: Option<String>,
        checked: bool,
        action: Action,
    },
    Separator,
}

impl Entry {
    fn height(&self, touch: bool) -> f32 {
        match self {
            Entry::Item { .. } if touch => TOUCH_ITEM,
            Entry::Item { .. } => ITEM,
            Entry::Separator => SEPARATOR,
        }
    }
}

/// An open context menu: its entries and where it was opened (widget
/// coordinates).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContextMenu {
    pub at: Point,
    pub entries: Vec<Entry>,
    /// Opened with a finger: taller entries, no keyboard shortcuts.
    pub touch: bool,
}

/// Shortcut text for Ctrl/Cmd + `key`.
fn shortcut(key: char) -> Option<String> {
    Some(if cfg!(target_os = "macos") {
        format!("\u{2318}{key}")
    } else {
        format!("Ctrl+{key}")
    })
}

/// The entries for the current selection.
pub(crate) fn entries(state: &EditorState) -> Vec<Entry> {
    let tr = |id| i18n::text(state.language, id);
    let visual = state.mode == Mode::Wysiwyg;
    let item = |id, hint: Option<String>, command| Entry::Item {
        label: tr(id),
        hint,
        checked: false,
        action: match command {
            // A dialog, as in word processors (not the link bar).
            Command::EditLink if visual => Action(Inner::LinkDialog),
            command => Action::from(command),
        },
    };
    let (a, b) = state.sel.range();
    let selected = a != b;
    let format = state.formatting();
    let features = state.features;
    let mut entries = Vec::new();

    if selected {
        entries.push(item("menu-cut", shortcut('X'), Command::Cut));
        entries.push(item("menu-copy", shortcut('C'), Command::Copy));
    }
    entries.push(item("menu-paste", shortcut('V'), Command::Paste));
    entries.push(item("menu-select-all", shortcut('A'), Command::SelectAll));

    if selected {
        entries.push(Entry::Separator);
        entries.push(item("menu-bold", shortcut('B'), Command::ToggleBold));
        entries.push(item("menu-italic", shortcut('I'), Command::ToggleItalic));
        if features.strikethrough {
            entries.push(item("menu-strike", shortcut('D'), Command::ToggleStrike));
        }
        entries.push(item("menu-code", shortcut('E'), Command::ToggleCode));
        entries.push(item("menu-link", shortcut('K'), Command::EditLink));
    } else if format.table && features.tables && visual {
        entries.push(Entry::Separator);
        entries.push(item(
            "menu-row-above",
            None,
            Command::InsertRow { below: false },
        ));
        entries.push(item(
            "menu-row-below",
            None,
            Command::InsertRow { below: true },
        ));
        entries.push(item(
            "menu-column-left",
            None,
            Command::InsertColumn { right: false },
        ));
        entries.push(item(
            "menu-column-right",
            None,
            Command::InsertColumn { right: true },
        ));
        entries.push(Entry::Separator);
        entries.push(item("menu-delete-row", None, Command::DeleteRow));
        entries.push(item("menu-delete-column", None, Command::DeleteColumn));
        entries.push(item("menu-delete-table", None, Command::DeleteTable));
        entries.push(Entry::Separator);
        let current = format.column_align.unwrap_or_default();
        for (id, align) in [
            ("menu-align-left", ColumnAlign::Left),
            ("menu-align-center", ColumnAlign::Center),
            ("menu-align-right", ColumnAlign::Right),
        ] {
            entries.push(Entry::Item {
                label: tr(id),
                hint: None,
                checked: current == align,
                action: Action::from(Command::SetColumnAlign(align)),
            });
        }
    }
    entries
}

/// Top-left corner of a menu of `size` opened at `at`, kept inside `area`.
/// It opens below and to the right of the pointer, and flips up or left
/// when there is no room.
fn place(at: Point, size: Size, area: Size) -> Point {
    let x = if at.x + size.width <= area.width {
        at.x
    } else {
        (area.width - size.width).max(0.0)
    };
    let y = if at.y + size.height <= area.height {
        at.y
    } else if at.y >= size.height {
        at.y - size.height
    } else {
        (area.height - size.height).max(0.0)
    };
    Point::new(x, y)
}

impl ContextMenu {
    /// The shortcut shown next to an entry (none for fingers).
    fn hint<'a>(&self, hint: &'a Option<String>) -> Option<&'a str> {
        hint.as_deref().filter(|_| !self.touch)
    }

    fn size(&self) -> Size {
        let widest = self
            .entries
            .iter()
            .map(|entry| match entry {
                Entry::Item { label, hint, .. } => {
                    let hint = self
                        .hint(hint)
                        .map_or(0.0, |hint| GAP + text_width(hint, fonts::SANS, HINT_SIZE));
                    CHECK + text_width(label, fonts::SANS, SIZE) + hint
                }
                Entry::Separator => 0.0,
            })
            .fold(0.0, f32::max);
        let width = (widest + 2.0 * (ITEM_PAD + PADDING)).max(MIN_WIDTH).ceil();
        let height = 2.0 * PADDING
            + self
                .entries
                .iter()
                .map(|entry| entry.height(self.touch))
                .sum::<f32>();
        Size::new(width, height)
    }

    /// Where the menu goes inside `area` (the widget).
    pub(crate) fn rect(&self, area: Size) -> Rectangle {
        let size = self.size();
        Rectangle::new(place(self.at, size, area), size)
    }

    /// Each entry with its rectangle.
    pub(crate) fn rows(&self, area: Size) -> impl Iterator<Item = (&Entry, Rectangle)> {
        let rect = self.rect(area);
        let mut y = rect.y + PADDING;
        let touch = self.touch;
        self.entries.iter().map(move |entry| {
            let height = entry.height(touch);
            let row = Rectangle::new(
                Point::new(rect.x + PADDING, y),
                Size::new(rect.width - 2.0 * PADDING, height),
            );
            y += height;
            (entry, row)
        })
    }

    /// The entry (an item, not a separator) under `point`.
    pub(crate) fn item_at(&self, area: Size, point: Point) -> Option<usize> {
        self.rows(area)
            .position(|(entry, row)| matches!(entry, Entry::Item { .. }) && row.contains(point))
    }

    /// The action of entry `index`.
    pub(crate) fn action(&self, index: usize) -> Option<Action> {
        match self.entries.get(index)? {
            Entry::Item { action, .. } => Some(action.clone()),
            Entry::Separator => None,
        }
    }

    pub(crate) fn draw(
        &self,
        frame: &mut Frame,
        area: Size,
        hovered: Option<usize>,
        palette: &Palette,
    ) {
        let rect = self.rect(area);
        // A soft shadow.
        for (grow, alpha) in [(3.0, 0.06), (1.5, 0.08)] {
            let shadow = Rectangle::new(
                Point::new(rect.x - grow, rect.y - grow + 3.0),
                Size::new(rect.width + 2.0 * grow, rect.height + 2.0 * grow),
            );
            rounded(
                frame,
                shadow,
                6.0 + grow,
                Some(Color::BLACK.scale_alpha(alpha)),
                None,
            );
        }
        rounded(
            frame,
            rect,
            6.0,
            Some(palette.toolbar_background),
            Some(palette.toolbar_border),
        );

        for (index, (entry, row)) in self.rows(area).enumerate() {
            match entry {
                Entry::Item {
                    label,
                    hint,
                    checked,
                    ..
                } => {
                    if hovered == Some(index) {
                        rounded(frame, row, 4.0, Some(palette.toolbar_hover), None);
                    }
                    let y = row.y + (row.height - SIZE * super::chrome::LINE) / 2.0;
                    if *checked {
                        draw_label(
                            frame,
                            "\u{2713}",
                            fonts::SANS,
                            SIZE,
                            Point::new(row.x + ITEM_PAD, y),
                            palette.toolbar_text,
                        );
                    }
                    draw_label(
                        frame,
                        label,
                        fonts::SANS,
                        SIZE,
                        Point::new(row.x + ITEM_PAD + CHECK, y),
                        palette.toolbar_text,
                    );
                    if let Some(hint) = self.hint(hint) {
                        let width = text_width(hint, fonts::SANS, HINT_SIZE);
                        let hint_y = row.y + (row.height - HINT_SIZE * super::chrome::LINE) / 2.0;
                        draw_label(
                            frame,
                            hint,
                            fonts::SANS,
                            HINT_SIZE,
                            Point::new(row.x + row.width - ITEM_PAD - width, hint_y),
                            palette.muted,
                        );
                    }
                }
                Entry::Separator => {
                    let y = (row.y + row.height / 2.0).floor() + 0.5;
                    frame.stroke(
                        &Path::line(
                            Point::new(row.x + 6.0, y),
                            Point::new(row.x + row.width - 6.0, y),
                        ),
                        Stroke::default()
                            .with_width(1.0)
                            .with_color(palette.toolbar_border),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_stays_inside_the_page() {
        let area = Size::new(800.0, 600.0);
        let size = Size::new(280.0, 200.0);
        assert_eq!(
            place(Point::new(10.0, 20.0), size, area),
            Point::new(10.0, 20.0)
        );
        // No room below: opens above the pointer.
        assert_eq!(
            place(Point::new(10.0, 500.0), size, area),
            Point::new(10.0, 300.0)
        );
        // No room on the right.
        assert_eq!(
            place(Point::new(700.0, 20.0), size, area),
            Point::new(520.0, 20.0)
        );
        // No room above or below.
        let small = Size::new(800.0, 250.0);
        assert_eq!(
            place(Point::new(0.0, 100.0), size, small),
            Point::new(0.0, 50.0)
        );
    }

    fn actions(state: &EditorState) -> Vec<Action> {
        entries(state)
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Item { action, .. } => Some(action),
                Entry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn selection_offers_formatting() {
        let mut state = EditorState::with_markdown("hello world\n");
        state.execute(Command::SelectAll);
        let actions = actions(&state);
        for command in [
            Command::Cut,
            Command::Copy,
            Command::ToggleBold,
            Command::ToggleItalic,
            Command::ToggleStrike,
            Command::ToggleCode,
        ] {
            assert!(
                actions.contains(&Action::from(command.clone())),
                "{command:?}"
            );
        }
        assert!(actions.contains(&Action(Inner::LinkDialog)));
        assert!(!actions.contains(&Action::from(Command::DeleteRow)));
    }

    #[test]
    fn source_mode_writes_the_link() {
        let mut state = EditorState::with_markdown("hello\n");
        state.execute(Command::SetMode(Mode::Source));
        state.execute(Command::SelectAll);
        let actions = actions(&state);
        assert!(actions.contains(&Action::from(Command::EditLink)));
        assert!(!actions.contains(&Action(Inner::LinkDialog)));
    }

    #[test]
    fn caret_in_a_table_offers_rows_and_columns() {
        let state = EditorState::with_markdown("| a | b |\n| - | - |\n");
        let actions = actions(&state);
        for command in [
            Command::InsertRow { below: false },
            Command::InsertRow { below: true },
            Command::InsertColumn { right: false },
            Command::InsertColumn { right: true },
            Command::DeleteRow,
            Command::DeleteColumn,
            Command::DeleteTable,
            Command::SetColumnAlign(ColumnAlign::Center),
        ] {
            assert!(
                actions.contains(&Action::from(command.clone())),
                "{command:?}"
            );
        }
        assert!(!actions.contains(&Action::from(Command::Cut)));
        assert!(!actions.contains(&Action::from(Command::ToggleBold)));
    }

    #[test]
    fn disabled_features_are_hidden() {
        let mut state = EditorState::with_markdown("a\n").features(crate::Features {
            strikethrough: false,
            ..crate::Features::default()
        });
        state.execute(Command::SelectAll);
        assert!(!actions(&state).contains(&Action::from(Command::ToggleStrike)));
    }

    #[test]
    fn touch_menus_have_taller_entries_and_no_shortcuts() {
        let mut state = EditorState::with_markdown("hello world\n");
        state.execute(Command::SelectAll);
        let mouse = ContextMenu {
            at: Point::new(10.0, 10.0),
            entries: entries(&state),
            touch: false,
        };
        let touch = ContextMenu {
            touch: true,
            ..mouse.clone()
        };
        let area = Size::new(800.0, 600.0);
        let (_, small) = mouse.rows(area).next().unwrap();
        let (_, large) = touch.rows(area).next().unwrap();
        assert!(large.height >= 40.0, "{large:?}");
        assert!(small.height < large.height);
        assert!(touch.size().width <= mouse.size().width);
        let Entry::Item { hint, .. } = &touch.entries[0] else {
            panic!("an item first");
        };
        assert!(hint.is_some() && touch.hint(hint).is_none());
    }

    #[test]
    fn separators_are_not_items() {
        let mut state = EditorState::with_markdown("hello world\n");
        state.execute(Command::SelectAll);
        let menu = ContextMenu {
            at: Point::new(10.0, 10.0),
            entries: entries(&state),
            touch: false,
        };
        let area = Size::new(800.0, 600.0);
        let separator = menu
            .entries
            .iter()
            .position(|entry| *entry == Entry::Separator)
            .expect("a separator");
        let (_, row) = menu.rows(area).nth(separator).unwrap();
        assert_eq!(menu.item_at(area, row.center()), None);
        let (_, first) = menu.rows(area).next().unwrap();
        assert_eq!(menu.item_at(area, first.center()), Some(0));
        assert_eq!(menu.action(0), Some(Action::from(Command::Cut)));
    }
}
