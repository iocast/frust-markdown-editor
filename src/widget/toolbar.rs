//! Contents of the built-in toolbar, link bar and status bar.

use super::chrome::{Bar, Button, Dropdown, Item, Pad};
use super::{Action, Command, EditorState, Formatting, Inner, Mode};
use crate::doc::{BlockKind, ColumnAlign};
use crate::fonts::{self, Face};
use crate::i18n::{self, Language};
use crate::style::Palette;

const TOOL_PAD: Pad = Pad::new(5.0, 9.0);
/// Smallest horizontal padding of a button used with a finger.
const TOUCH_PAD_X: f32 = 12.0;

/// Larger targets for fingers: `pad_y` above and below the labels of
/// buttons and drop-downs.
fn for_touch(items: &mut [Item], pad_y: f32) {
    for item in items {
        match item {
            Item::Button(button) => {
                button.pad = Pad::new(pad_y.max(button.pad.y), button.pad.x.max(TOUCH_PAD_X));
            }
            Item::Dropdown(dropdown) => dropdown.pad.y = pad_y.max(dropdown.pad.y),
            Item::Row { items, .. } => for_touch(items, pad_y),
            _ => {}
        }
    }
}

fn tool(label: impl Into<String>, face: Face, on: bool, command: Option<Command>) -> Item {
    Item::Button(Button {
        label: label.into(),
        face,
        size: 13.0,
        pad: TOOL_PAD,
        on,
        action: command.map(Action::from),
    })
}

/// Language selector shown while the caret is in a code block.
fn language_list(format: &Formatting, languages: Vec<String>, ui: Language) -> Item {
    // Entry that means "no language".
    let plain = i18n::text(ui, "code-plain-text");
    let mut names = vec![plain.clone()];
    names.extend(languages);
    let current = format
        .code_language
        .clone()
        .unwrap_or_else(|| plain.clone());
    if !names.contains(&current) {
        names.push(current.clone());
    }
    let options = names
        .into_iter()
        .map(|name| {
            let language = (name != plain).then(|| name.clone());
            (name, Action::from(Command::SetCodeLanguage(language)))
        })
        .collect();
    Item::Dropdown(Dropdown {
        current,
        options,
        width: 130.0,
        size: 12.0,
        pad: Pad::new(3.0, 6.0),
    })
}

/// The formatting toolbar.
pub(crate) fn toolbar(state: &EditorState) -> Bar {
    let format = &state.formatting();
    let features = state.features;
    let (can_undo, can_redo) = (state.can_undo(), state.can_redo());
    let visual = state.mode == Mode::Wysiwyg;
    let block = format.block;
    let ui = fonts::SANS;
    let task = format.task.is_some();
    let set = |kind| Some(Command::SetBlock(kind));
    let tr = |id| i18n::text(state.language, id);

    let mut items = vec![
        tool("\u{21B6}", ui, false, can_undo.then_some(Command::Undo)),
        tool("\u{21B7}", ui, false, can_redo.then_some(Command::Redo)),
        Item::Separator,
        tool(
            tr("toolbar-normal"),
            ui,
            visual && block == BlockKind::Paragraph,
            set(BlockKind::Paragraph),
        ),
        tool(
            "H1",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(1),
            set(BlockKind::Heading(1)),
        ),
        tool(
            "H2",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(2),
            set(BlockKind::Heading(2)),
        ),
        tool(
            "H3",
            fonts::SANS_BOLD,
            visual && block == BlockKind::Heading(3),
            set(BlockKind::Heading(3)),
        ),
        Item::Separator,
        tool(
            tr("toolbar-bold"),
            fonts::SANS_BOLD,
            visual && format.bold,
            Some(Command::ToggleBold),
        ),
        tool(
            tr("toolbar-italic"),
            fonts::SANS_ITALIC,
            visual && format.italic,
            Some(Command::ToggleItalic),
        ),
        tool(
            "</>",
            fonts::MONO,
            visual && format.code,
            Some(Command::ToggleCode),
        ),
        tool(
            tr("toolbar-link"),
            ui,
            visual && format.link.is_some(),
            Some(Command::EditLink),
        ),
    ];

    if features.strikethrough {
        items.push(tool(
            tr("toolbar-strike"),
            ui,
            visual && format.strike,
            Some(Command::ToggleStrike),
        ));
    }

    if state.image_settings.enabled() {
        items.push(tool(
            tr("toolbar-image"),
            ui,
            false,
            Some(Command::RequestImage),
        ));
    }

    items.push(Item::Separator);
    items.push(tool(
        "\u{2022} \u{2014}",
        ui,
        visual && !task && format.list == Some(false),
        set(BlockKind::Bullet),
    ));
    items.push(tool(
        "1. \u{2014}",
        ui,
        visual && !task && format.list == Some(true),
        set(BlockKind::Ordered),
    ));

    if features.task_lists {
        items.push(tool(
            "\u{2611} \u{2014}",
            ui,
            visual && task,
            set(BlockKind::Task),
        ));
    }

    items.push(tool(
        "\u{201C} \u{201D}",
        ui,
        visual && format.quote,
        set(BlockKind::Quote),
    ));
    items.push(tool(
        "{ }",
        ui,
        visual && block == BlockKind::Code,
        set(BlockKind::Code),
    ));

    if visual && block == BlockKind::Code {
        items.push(language_list(
            format,
            state.code_languages(),
            state.language,
        ));
    }

    items.push(tool("\u{2015}", ui, false, Some(Command::InsertRule)));

    if features.tables {
        items.push(tool(
            tr("toolbar-table"),
            ui,
            visual && format.table,
            (!format.table).then_some(Command::InsertTable {
                rows: 3,
                columns: 3,
            }),
        ));
    }

    if visual && format.table && features.tables {
        let align = format.column_align.unwrap_or_default();
        items.extend([
            tool(
                tr("toolbar-add-row"),
                ui,
                false,
                Some(Command::InsertRow { below: true }),
            ),
            tool(
                tr("toolbar-add-column"),
                ui,
                false,
                Some(Command::InsertColumn { right: true }),
            ),
            tool(
                tr("toolbar-delete-row"),
                ui,
                false,
                Some(Command::DeleteRow),
            ),
            tool(
                tr("toolbar-delete-column"),
                ui,
                false,
                Some(Command::DeleteColumn),
            ),
            tool(
                "\u{21E4}",
                ui,
                align == ColumnAlign::Left,
                Some(Command::SetColumnAlign(ColumnAlign::Left)),
            ),
            tool(
                "\u{2194}",
                ui,
                align == ColumnAlign::Center,
                Some(Command::SetColumnAlign(ColumnAlign::Center)),
            ),
            tool(
                "\u{21E5}",
                ui,
                align == ColumnAlign::Right,
                Some(Command::SetColumnAlign(ColumnAlign::Right)),
            ),
        ]);
    }

    items.push(Item::Separator);
    items.push(tool(
        "WYSIWYG",
        ui,
        visual,
        Some(Command::SetMode(Mode::Wysiwyg)),
    ));
    items.push(tool(
        tr("toolbar-source"),
        ui,
        !visual,
        Some(Command::SetMode(Mode::Source)),
    ));

    // About 40 pixels high: easy to hit with a finger.
    if state.touch {
        for_touch(&mut items, 10.0);
    }

    let palette = state.style.palette;
    Bar {
        items,
        padding: Pad::new(4.0, 8.0),
        spacing: if state.touch { 4.0 } else { 2.0 },
        // Narrow editors: buttons continue on the next line instead of being cut off.
        wrap: true,
        background: palette.toolbar_background,
        border: Some(palette.toolbar_border),
    }
}

fn small_button(label: String, action: Action, on: bool, pad: Pad) -> Item {
    Item::Button(Button {
        label,
        face: fonts::SANS,
        size: 12.0,
        pad,
        on,
        action: Some(action),
    })
}

/// Width of the link bar's address field.
pub(crate) const LINK_INPUT_WIDTH: f32 = 420.0;

/// The bar for editing a link address; the text field is a Frust widget
/// placed in the `Input` slot. `touch`: larger buttons for fingers.
pub(crate) fn link_bar(palette: &Palette, language: Language, touch: bool) -> Bar {
    let tr = |id| i18n::text(language, id);
    let pad = Pad::new(5.0, 10.0);
    let mut items = vec![
        Item::Label {
            text: tr("link-address"),
            size: 12.0,
            color: palette.muted,
        },
        Item::Input {
            width: LINK_INPUT_WIDTH,
        },
        small_button(tr("link-apply"), Action(Inner::LinkApply), true, pad),
        small_button(tr("link-remove"), Action(Inner::LinkRemove), false, pad),
        small_button(tr("link-cancel"), Action(Inner::LinkCancel), false, pad),
    ];
    if touch {
        for_touch(&mut items, 10.0);
    }
    Bar {
        items,
        padding: Pad::new(6.0, 12.0),
        spacing: 8.0,
        wrap: true,
        background: palette.toolbar_background,
        border: Some(palette.toolbar_border),
    }
}

/// Page, word count, tasks, mode, Tab width, modified flag and zoom.
pub(crate) fn status_bar(state: &EditorState) -> Bar {
    let language = state.language;
    let palette = state.style.palette;
    let tr = |id| i18n::text(language, id);
    let label = |text: String| Item::Label {
        text,
        size: 12.0,
        color: palette.muted,
    };
    let mut items = Vec::new();

    if !state.paper.is_fill() {
        items.push(label(i18n::text_with(
            language,
            "status-page",
            &[
                ("current", state.current_page().into()),
                ("total", state.page_count().into()),
            ],
        )));
    }

    items.push(label(i18n::text_with(
        language,
        "status-words",
        &[("count", state.word_count().into())],
    )));

    let (checked, total) = state.task_progress();
    if total > 0 {
        items.push(label(i18n::text_with(
            language,
            "status-tasks",
            &[("checked", checked.into()), ("total", total.into())],
        )));
    }

    let pending = state.pending_images();
    if pending > 0 {
        items.push(label(i18n::text_with(
            language,
            "status-saving-images",
            &[("count", pending.into())],
        )));
    }

    items.push(label(match state.mode {
        Mode::Wysiwyg => tr("status-mode-wysiwyg"),
        Mode::Source => tr("status-mode-source"),
    }));

    if state.tab_width > 0 {
        items.push(label(i18n::text_with(
            language,
            "status-tab-width",
            &[("count", state.tab_width.into())],
        )));
    }

    items.push(Item::Fill);
    if state.dirty {
        items.push(label(tr("status-modified")));
    }

    let zoom = &state.zoom_settings;
    if zoom.enabled && zoom.controls {
        let pad = Pad::new(1.0, 8.0);
        let percent = format!("{:.0} %", state.zoom * 100.0);
        items.push(Item::Row {
            items: vec![
                small_button("\u{2212}".into(), Command::ZoomOut.into(), false, pad),
                small_button(percent, Command::ResetZoom.into(), false, pad),
                small_button("+".into(), Command::ZoomIn.into(), false, pad),
            ],
            spacing: 2.0,
        });
    }
    if state.touch {
        // Kept smaller than in the toolbar: the status bar stays slim.
        for_touch(&mut items, 6.0);
    }

    Bar {
        items,
        padding: Pad::new(3.0, 12.0),
        spacing: 18.0,
        wrap: false,
        background: palette.toolbar_background,
        border: None,
    }
}
