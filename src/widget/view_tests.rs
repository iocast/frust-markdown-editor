//! Headless interaction tests: the real widget inside Frust's `RenderRoot`,
//! fed with the same input events the desktop shell sends.

use super::*;
use crate::page::Paper;
use crate::widget::Event;
use frust::authoring::text::TextContext;
use frust::authoring::{Key as FrustKey, KeyEvent, Modifiers as FrustModifiers, NamedKey};
use frust::kurbo;
use frust_core::{FrameTime, RenderRoot};
use std::any::Any;

const W: f64 = 1000.0;
const H: f64 = 700.0;

struct App {
    editor: EditorState,
    events: Vec<Event>,
    clicked: u32,
}

impl App {
    fn new(markdown: &str) -> App {
        App {
            editor: EditorState::with_markdown(markdown).paper(Paper::fill()),
            events: Vec::new(),
            clicked: 0,
        }
    }
}

fn editor_view(app: &mut App) -> EditorView<App> {
    markdown_editor(&app.editor).view(|app: &mut App, action| {
        let output = app.editor.update(action);
        app.events.extend(output.events);
    })
}

/// Records nothing; paint runs for its side effects.
struct Discard;

impl PaintScene for Discard {
    fn fill_rect(&mut self, _: kurbo::Point, _: kurbo::Size, _: frust::Color) {}
    fn draw_text(&mut self, _: kurbo::Point, _: &str) {}
}

struct Harness<V: View<App>, F: FnMut(&mut App) -> V> {
    root: RenderRoot<App, V>,
    logic: F,
    app: App,
    text: TextContext,
    /// The window.
    size: kurbo::Size,
}

impl<V: View<App>, F: FnMut(&mut App) -> V> Harness<V, F> {
    fn new(app: App, logic: F) -> Self {
        Self::sized(app, logic, W, H)
    }

    fn sized(app: App, logic: F, width: f64, height: f64) -> Self {
        let mut harness = Harness {
            root: RenderRoot::new(),
            logic,
            app,
            text: TextContext::new(),
            size: kurbo::Size::new(width, height),
        };
        // Two frames: the second picks up the measured size.
        harness.frame();
        harness.frame();
        harness
    }

    fn frame(&mut self) {
        self.root.rebuild(&mut self.logic, &mut self.app);
        let text: &mut dyn Any = &mut self.text;
        self.root.layout_with_text(self.size, text);
        self.root.paint(&mut Discard, FrameTime::ZERO);
    }

    /// Finger `slot` (0: the first one) touching, moving or lifted.
    fn finger(&mut self, slot: u32, phase: PointerPhase, x: f32, y: f32) {
        self.event(InputEvent::PointerContact {
            pointer_id: frust_core::event::PointerId::touch(slot),
            event: PointerEvent {
                phase,
                position: kurbo::Point::new(x as f64, y as f64),
                button: PointerButton::Primary,
            },
        });
    }

    fn tap(&mut self, x: f32, y: f32) {
        self.finger(0, PointerPhase::Down, x, y);
        self.finger(0, PointerPhase::Up, x, y);
    }

    /// One finger from `from` to `to`, in a few steps.
    fn swipe(&mut self, from: (f32, f32), to: (f32, f32)) {
        self.finger(0, PointerPhase::Down, from.0, from.1);
        for step in 1..=4 {
            let t = step as f32 / 4.0;
            let x = from.0 + (to.0 - from.0) * t;
            let y = from.1 + (to.1 - from.1) * t;
            self.finger(0, PointerPhase::Move, x, y);
        }
        self.finger(0, PointerPhase::Up, to.0, to.1);
    }

    fn event(&mut self, event: InputEvent) {
        self.root.event(&mut self.app, &event);
        self.frame();
    }

    fn click(&mut self, x: f32, y: f32) {
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            self.event(InputEvent::Pointer(PointerEvent {
                phase,
                position: kurbo::Point::new(x as f64, y as f64),
                button: PointerButton::Primary,
            }));
        }
    }

    fn key(&mut self, key: FrustKey, modifiers: FrustModifiers) {
        self.event(InputEvent::Key(KeyEvent {
            key,
            modifiers,
            repeat: false,
        }));
    }

    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.key(
                FrustKey::Character(c.to_string()),
                FrustModifiers::default(),
            );
        }
    }

    fn shortcut(&mut self, c: &str) {
        self.key(FrustKey::Character(c.into()), command());
    }

    fn named(&mut self, key: NamedKey) {
        self.key(FrustKey::Named(key), FrustModifiers::default());
    }

    /// Height of the toolbar as laid out at window width.
    fn toolbar(&self) -> (f32, Vec<chrome::Slot>) {
        chrome::layout(
            &toolbar::toolbar(&self.app.editor),
            Point::ORIGIN,
            self.size.width as f32,
            0.0,
        )
    }

    /// Centre of the toolbar button labelled `label`.
    fn tool(&self, label: &str) -> Point {
        let (_, slots) = self.toolbar();
        let slot = slots
            .iter()
            .find(|slot| matches!(&slot.item, Item::Button(b) if b.label == label))
            .unwrap_or_else(|| panic!("no toolbar button {label:?}"));
        slot.rect.center()
    }

    /// A point inside the page, below the toolbar.
    fn in_page(&self) -> (f32, f32) {
        (300.0, self.toolbar().0 + 30.0)
    }
}

fn command() -> FrustModifiers {
    FrustModifiers {
        ctrl: !cfg!(target_os = "macos"),
        meta: cfg!(target_os = "macos"),
        ..FrustModifiers::default()
    }
}

fn editor_alone(
    markdown: &str,
) -> Harness<EditorView<App>, impl FnMut(&mut App) -> EditorView<App>> {
    Harness::new(App::new(markdown), editor_view)
}

#[test]
fn clicking_focuses_and_typing_edits() {
    let mut h = editor_alone("hello\n");
    assert!(!h.app.editor.is_focused());
    let (x, y) = h.in_page();
    h.click(x, y);
    assert!(h.app.editor.is_focused());
    assert!(h.app.events.contains(&Event::Focused));

    h.shortcut("a");
    h.type_text("Frust");
    h.named(NamedKey::Enter);
    h.type_text("two");
    assert_eq!(h.app.editor.markdown(), "Frust\n\ntwo\n");
    assert!(h.app.events.contains(&Event::Changed));
}

#[test]
fn the_widget_reports_its_size_to_the_layout() {
    let h = editor_alone("text\n");
    let viewport = h.app.editor.viewport.get();
    assert_eq!(viewport.width, W as f32);
    // Window minus toolbar and status bar.
    assert!(viewport.height < H as f32 - 40.0, "{viewport:?}");
    assert!(viewport.height > H as f32 - 120.0, "{viewport:?}");
}

#[test]
fn shortcuts_and_toolbar_format() {
    let mut h = editor_alone("word\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    h.shortcut("a");
    h.shortcut("b");
    assert_eq!(h.app.editor.markdown(), "**word**\n");

    // Toolbar buttons act and keep the keyboard focus in the page.
    let heading = h.tool("H1");
    h.click(heading.x, heading.y);
    assert_eq!(h.app.editor.markdown(), "# **word**\n");
    assert!(h.app.editor.is_focused());
    h.named(NamedKey::End);
    h.type_text("s");
    assert_eq!(h.app.editor.markdown(), "# **words**\n");
}

#[test]
fn copy_cut_and_paste_go_through_frust() {
    let mut h = editor_alone("copy me\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    h.shortcut("a");
    h.shortcut("c");
    assert_eq!(h.root.take_clipboard_write().as_deref(), Some("copy me"));

    h.shortcut("x");
    assert_eq!(h.root.take_clipboard_write().as_deref(), Some("copy me"));
    assert_eq!(
        h.app.editor.markdown(),
        "
"
    );

    h.shortcut("v");
    assert!(
        h.root.take_paste_request(),
        "the editor asked for the clipboard"
    );
    // The shell answers with the clipboard text.
    h.event(InputEvent::EditCommand(EditCommand::Paste("pasted".into())));
    assert_eq!(h.app.editor.markdown(), "pasted\n");
}

#[test]
fn commands_run_by_the_application_reach_the_clipboard() {
    let mut h = editor_alone("app copy\n");
    h.app.editor.execute(Command::SelectAll);
    h.app.editor.execute(Command::Copy);
    h.frame();
    assert_eq!(h.root.take_clipboard_write().as_deref(), Some("app copy"));
}

#[test]
fn focus_command_claims_the_keyboard() {
    let mut h = editor_alone("x\n");
    h.app.editor.execute(Command::Focus);
    h.frame();
    h.frame();
    h.type_text("y");
    assert_eq!(h.app.editor.markdown(), "yx\n");
}

#[test]
fn clicking_another_widget_blurs_the_editor() {
    let logic = |app: &mut App| {
        frust::column::<App>()
            .child(frust::button("Other", |app: &mut App| app.clicked += 1))
            .flex(1, editor_view(app))
    };
    let mut h = Harness::new(App::new("text\n"), logic);
    // The button sits at the top; the editor below it.
    h.click(300.0, 200.0);
    assert!(h.app.editor.is_focused());
    h.click(10.0, 10.0);
    assert_eq!(h.app.clicked, 1);
    h.frame();
    assert!(!h.app.editor.is_focused());
    assert!(h.app.events.contains(&Event::Blurred));
}

#[test]
fn wheel_scrolls_the_page() {
    let text: String = (0..200).map(|i| format!("line {i}\n\n")).collect();
    let mut h = editor_alone(&text);
    let (x, y) = h.in_page();
    h.event(InputEvent::Scroll {
        position: kurbo::Point::new(x as f64, y as f64),
        delta: frust::authoring::ScrollDelta::Lines(0.0, 3.0),
    });
    assert!(
        (h.app.editor.scroll_y - 3.0 * LINE).abs() < 0.5,
        "{}",
        h.app.editor.scroll_y
    );
}

#[test]
fn ctrl_wheel_steps_the_zoom() {
    let mut h = editor_alone("zoom\n");
    let (x, y) = h.in_page();
    h.event(InputEvent::Scale(frust_core::event::ScaleEvent {
        phase: frust_core::event::ScalePhase::Update,
        scale_delta: (0.1f64).exp(),
        focal: kurbo::Point::new(x as f64, y as f64),
        velocity: 0.0,
    }));
    assert_eq!(h.app.editor.current_zoom(), 1.1);
}

#[test]
fn link_bar_edits_the_address() {
    let mut h = editor_alone("site\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    h.shortcut("k");
    assert!(h.app.editor.link_url.is_some(), "the link bar is open");

    // The address field sits in the link bar; click it and type.
    let field = {
        let toolbar = h.toolbar().0;
        let bar = toolbar::link_bar(&h.app.editor.style.palette, h.app.editor.language, false);
        let (_, slots) = chrome::layout(&bar, Point::new(0.0, toolbar), W as f32, 30.0);
        slots
            .iter()
            .find(|slot| matches!(slot.item, Item::Input { .. }))
            .expect("input slot")
            .rect
            .center()
    };
    h.click(field.x, field.y);
    h.named(NamedKey::End);
    h.type_text("frust.dev");
    assert_eq!(h.app.editor.link_url.as_deref(), Some("https://frust.dev"));
    h.named(NamedKey::Enter);
    assert!(h.app.editor.link_url.is_none(), "the link bar closed");
    assert_eq!(h.app.editor.markdown(), "[site](https://frust.dev)\n");

    // Typing goes to the page again.
    h.frame();
    h.named(NamedKey::End);
    h.type_text("s");
    assert_eq!(h.app.editor.markdown(), "[site](https://frust.dev)s\n");
}

#[test]
fn code_language_drop_down() {
    let mut h = editor_alone("```\ncode\n```\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    let (_, slots) = h.toolbar();
    let dropdown = slots
        .iter()
        .find(|slot| matches!(slot.item, Item::Dropdown(_)))
        .expect("language list while in a code block")
        .clone();
    let Item::Dropdown(list) = &dropdown.item else {
        unreachable!()
    };
    let rust = list
        .options
        .iter()
        .position(|(name, _)| name == "rust")
        .expect("rust is offered");
    let at = dropdown.rect.center();
    h.click(at.x, at.y);
    // Scroll the list until `rust` is visible, then pick it.
    let menu = chrome::menu(
        list,
        dropdown.rect,
        Rectangle::with_size(Size::new(W as f32, H as f32)),
        0,
    );
    let first = rust.saturating_sub(menu.visible - 1);
    for _ in 0..first {
        h.event(InputEvent::Scroll {
            position: kurbo::Point::new(at.x as f64, (menu.area.y + 5.0) as f64),
            delta: frust::authoring::ScrollDelta::Lines(0.0, 1.0),
        });
    }
    let row = (rust - first) as f32;
    let target_y = menu.area.y + (row + 0.5) * list.option_height();
    let event = |phase| {
        InputEvent::Pointer(PointerEvent {
            phase,
            position: kurbo::Point::new(at.x as f64, target_y as f64),
            button: PointerButton::Primary,
        })
    };
    h.event(event(PointerPhase::Down));
    h.event(event(PointerPhase::Up));
    assert_eq!(h.app.editor.markdown(), "```rust\ncode\n```\n");
}

#[test]
fn dropped_image_files_are_offered_to_the_editor() {
    let mut app = App::new("");
    app.editor
        .set_image_settings(crate::ImageSettings::embed().accept_drops(true));
    let mut h = Harness::new(app, editor_view);
    let (x, y) = h.in_page();
    h.event(InputEvent::FileDrop(frust::FileDropEvent {
        phase: FileDropPhase::Drop,
        position: kurbo::Point::new(x as f64, y as f64),
        paths: vec!["missing-file.png".into()],
    }));
    assert!(
        h.app
            .events
            .iter()
            .any(|event| matches!(event, Event::ImageRejected { .. })),
        "{:?}",
        h.app.events
    );
}

impl<V: View<App>, F: FnMut(&mut App) -> V> Harness<V, F> {
    fn right_click(&mut self, x: f32, y: f32) {
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            self.event(InputEvent::Pointer(PointerEvent {
                phase,
                position: kurbo::Point::new(x as f64, y as f64),
                button: PointerButton::Secondary,
            }));
        }
    }

    /// Centre of the context menu entry labelled `label`.
    fn menu_item(&self, label: &str) -> Point {
        let editor = &self.app.editor;
        let menu = ContextMenu {
            at: editor.context_menu.expect("the context menu is open"),
            entries: super::super::menu::entries(editor),
            touch: editor.touch,
        };
        let (_, row) = menu
            .rows(Size::new(self.size.width as f32, self.size.height as f32))
            .find(|(entry, _)| {
                matches!(entry, super::super::menu::Entry::Item { label: l, .. } if l == label)
            })
            .unwrap_or_else(|| panic!("no menu entry {label:?}"));
        row.center()
    }

    /// Selects everything, right-clicks it and picks "Link…".
    fn open_link_dialog(&mut self) {
        let (x, y) = self.in_page();
        self.click(x, y);
        self.shortcut("a");
        self.right_click(x, y);
        let link = self.menu_item("Link…");
        self.click(link.x, link.y);
        // The dialog takes the focus on the next frames.
        self.frame();
        self.frame();
        assert!(self.app.editor.link_dialog.is_some(), "the dialog is open");
    }
}

#[test]
fn right_click_formats_the_selection() {
    let mut h = editor_alone("hello world\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    h.shortcut("a");
    h.right_click(x, y);
    assert!(h.app.editor.context_menu.is_some(), "the menu is open");
    assert_eq!(h.app.editor.selected_text().as_deref(), Some("hello world"));

    let bold = h.menu_item("Bold");
    h.click(bold.x, bold.y);
    assert!(h.app.editor.context_menu.is_none(), "the menu closed");
    assert_eq!(h.app.editor.markdown(), "**hello world**\n");
    // Typing goes to the page again.
    h.named(NamedKey::End);
    h.type_text("s");
    assert_eq!(h.app.editor.markdown(), "**hello worlds**\n");
}

#[test]
fn right_click_in_a_table_inserts_a_column() {
    let mut h = editor_alone("| a | b |\n| - | - |\n");
    let (x, y) = h.in_page();
    h.right_click(x, y);
    let left = h.menu_item("Insert column left");
    h.click(left.x, left.y);
    let markdown = h.app.editor.markdown();
    assert_eq!(
        markdown.lines().next().unwrap().matches('|').count(),
        4,
        "{markdown}"
    );
}

#[test]
fn escape_and_clicks_elsewhere_close_the_menu() {
    let mut h = editor_alone("text\n");
    let (x, y) = h.in_page();
    h.right_click(x, y);
    assert!(h.app.editor.context_menu.is_some());
    h.named(NamedKey::Escape);
    assert!(h.app.editor.context_menu.is_none());
    assert_eq!(h.app.editor.markdown(), "text\n");

    h.right_click(x, y);
    // Far from the menu (it opens below and right of the pointer).
    h.click(x - 200.0, y);
    assert!(h.app.editor.context_menu.is_none());
}

#[test]
fn context_menu_can_be_disabled() {
    let logic = |app: &mut App| {
        markdown_editor(&app.editor)
            .context_menu(false)
            .view(|app: &mut App, action| {
                app.editor.update(action);
            })
    };
    let mut h = Harness::new(App::new("text\n"), logic);
    let (x, y) = h.in_page();
    h.right_click(x, y);
    assert!(h.app.editor.context_menu.is_none());
}

#[test]
fn link_dialog_types_the_address_and_the_text() {
    let mut h = editor_alone("site\n");
    h.open_link_dialog();

    // The address field has the focus, its content selected.
    h.type_text("https://frust.dev");
    assert_eq!(h.app.editor.link_url.as_deref(), Some("https://frust.dev"));
    // Tab: the text to display, selected too.
    h.named(NamedKey::Tab);
    h.type_text("Frust");
    h.named(NamedKey::Tab);
    h.named(NamedKey::End);
    h.type_text("/docs");
    h.named(NamedKey::Enter);
    assert!(h.app.editor.link_dialog.is_none(), "the dialog closed");
    assert_eq!(h.app.editor.markdown(), "[Frust](https://frust.dev/docs)\n");

    // Typing goes to the page again.
    h.frame();
    h.type_text("s");
    assert_eq!(
        h.app.editor.markdown(),
        "[Frust](https://frust.dev/docs)s\n"
    );
}

#[test]
fn link_dialog_is_modal_and_escape_cancels() {
    let mut h = editor_alone("site\n");
    h.open_link_dialog();
    // Clicks and keys do not reach the page behind.
    let (x, y) = h.in_page();
    h.click(x, y);
    assert!(h.app.editor.link_dialog.is_some());
    h.named(NamedKey::Escape);
    assert!(h.app.editor.link_dialog.is_none());
    assert!(h.app.editor.link_url.is_none());
    assert_eq!(h.app.editor.markdown(), "site\n");
    h.frame();
    h.type_text("x");
    assert_eq!(h.app.editor.markdown(), "x\n");
}

// ------------------------------------------------------------------------
// Touch screens and narrow windows
// ------------------------------------------------------------------------

fn long_document() -> String {
    (1..=80).map(|i| format!("Line {i}\n\n")).collect()
}

impl<V: View<App>, F: FnMut(&mut App) -> V> Harness<V, F> {
    /// Window position of the caret at `offset` in the first paragraph.
    fn at_offset(&self, offset: usize) -> (f32, f32) {
        let (x, y, height) = self
            .app
            .editor
            .layout
            .borrow()
            .caret(crate::doc::Pos::new(0, offset))
            .expect("a caret position");
        (x, self.toolbar().0 + y + height / 2.0)
    }

    fn ime(&mut self, ime: ImeEvent) {
        self.event(InputEvent::Ime(ime));
    }

    /// As after a first touch: the bars are sized for fingers.
    fn touch_mode(&mut self) {
        self.app.editor.touch = true;
        self.frame();
    }
}

#[test]
fn a_finger_drag_scrolls_the_page_instead_of_selecting() {
    let document = long_document();
    let mut h = editor_alone(&document);
    let (x, y) = h.in_page();
    h.swipe((x, y + 300.0), (x, y));
    let editor = &h.app.editor;
    assert!((editor.scroll_y - 300.0).abs() < 1.0, "{}", editor.scroll_y);
    assert_eq!(editor.sel.anchor, editor.sel.head, "nothing selected");
    assert!(!editor.is_focused(), "scrolling does not open the keyboard");
    assert!(editor.touch);

    // And back (the toolbar has grown for fingers meanwhile).
    let (x, y) = h.in_page();
    h.swipe((x, y), (x, y + 100.0));
    assert!((h.app.editor.scroll_y - 200.0).abs() < 1.0);
}

#[test]
fn a_tap_places_the_caret_and_starts_text_input() {
    let mut h = editor_alone("hello world\n");
    h.touch_mode();
    let (x, y) = h.in_page();
    h.tap(x, y);
    assert!(h.app.editor.is_focused());
    assert!(
        h.root.ime_state().is_some_and(|ime| ime.active),
        "the on-screen keyboard is asked for"
    );
    // Right of the text: the caret goes to the end of the line.
    h.type_text("s");
    assert_eq!(h.app.editor.markdown(), "hello worlds\n");
}

#[test]
fn a_double_tap_selects_a_word() {
    let mut h = editor_alone("hello world\n");
    h.touch_mode();
    let (x, y) = h.at_offset(2);
    h.tap(x, y);
    h.tap(x, y);
    assert_eq!(h.app.editor.selected_text().as_deref(), Some("hello"));
}

#[test]
fn a_long_press_selects_and_offers_the_menu() {
    let mut h = editor_alone("hello world\n");
    h.touch_mode();
    let (x, y) = h.at_offset(2);
    h.finger(0, PointerPhase::Down, x, y);
    std::thread::sleep(LONG_PRESS + Duration::from_millis(50));
    h.finger(0, PointerPhase::Up, x, y);
    assert_eq!(h.app.editor.selected_text().as_deref(), Some("hello"));
    assert!(h.app.editor.context_menu.is_some(), "the menu is open");

    // Its entries are made for fingers.
    let bold = h.menu_item("Bold");
    h.tap(bold.x, bold.y);
    assert_eq!(h.app.editor.markdown(), "**hello** world\n");

    // Held, then moved: selects from where the finger went down.
    let (start, y) = h.at_offset(0);
    let (end, _) = h.at_offset(h.app.editor.doc.blocks[0].len());
    h.finger(0, PointerPhase::Down, start + 1.0, y);
    std::thread::sleep(LONG_PRESS + Duration::from_millis(50));
    h.finger(0, PointerPhase::Move, (start + end) / 2.0, y);
    h.finger(0, PointerPhase::Move, end + 20.0, y);
    h.finger(0, PointerPhase::Up, end + 20.0, y);
    assert_eq!(h.app.editor.selected_text().as_deref(), Some("hello world"));
}

#[test]
fn two_fingers_zoom() {
    let document = long_document();
    let mut h = editor_alone(&document);
    let (x, y) = h.in_page();
    h.finger(0, PointerPhase::Down, x, y + 100.0);
    h.finger(1, PointerPhase::Down, x, y + 200.0);
    h.finger(1, PointerPhase::Move, x, y + 250.0);
    h.finger(1, PointerPhase::Move, x, y + 300.0);
    h.finger(1, PointerPhase::Up, x, y + 300.0);
    h.finger(0, PointerPhase::Up, x, y + 100.0);
    let zoom = h.app.editor.zoom;
    assert!((zoom - 2.0).abs() < 0.05, "{zoom}");
    assert!(!h.app.editor.is_focused(), "a pinch is not a tap");
}

#[test]
fn touch_makes_bars_and_menus_larger() {
    let mut h = editor_alone("text\n");
    let mouse = h.toolbar().0;
    let (x, y) = h.in_page();
    h.tap(x, y);
    assert!(h.app.editor.touch);
    assert!(h.toolbar().0 > mouse + 8.0, "{} vs {mouse}", h.toolbar().0);
    // A mouse again: the usual sizes.
    h.click(x, y);
    assert!(!h.app.editor.touch);
    assert_eq!(h.toolbar().0, mouse);
}

#[test]
fn composed_text_is_shown_and_committed_as_one_undo_step() {
    let mut h = editor_alone("x\n");
    let (x, y) = h.in_page();
    h.click(x, y);
    let compose = |text: &str| ImeEvent::Compose {
        text: text.into(),
        cursor: None,
    };
    h.ime(compose("wor"));
    assert_eq!(h.app.editor.markdown(), "xwor\n");
    h.ime(compose("word"));
    assert_eq!(h.app.editor.markdown(), "xword\n");
    h.ime(ImeEvent::Commit("words".into()));
    assert_eq!(h.app.editor.markdown(), "xwords\n");
    h.type_text("s");
    assert_eq!(h.app.editor.markdown(), "xwordss\n");
    h.shortcut("z");
    assert_eq!(h.app.editor.markdown(), "x\n");

    // A cancelled composition leaves nothing.
    h.ime(compose("abc"));
    h.ime(compose(""));
    assert_eq!(h.app.editor.markdown(), "x\n");

    // A session that ends keeps what was composed.
    h.ime(compose("abc"));
    h.ime(ImeEvent::Disabled);
    h.ime(compose("de"));
    h.ime(ImeEvent::Commit("de".into()));
    assert_eq!(h.app.editor.markdown(), "xabcde\n");
}

#[test]
fn a_narrow_toolbar_is_one_row_that_scrolls_sideways() {
    let width = 380.0;
    let mut h = Harness::sized(App::new("text\n"), editor_view, width, H);
    let bar = toolbar::toolbar(&h.app.editor);
    assert!(chrome::line_count(&bar, width as f32, 0.0) > TOOLBAR_LINES);
    let row = Bar { wrap: false, ..bar };
    let content = chrome::natural_width(&row, 0.0);
    let (row_height, _) = chrome::layout(&row, Point::ORIGIN, content, 0.0);
    // The page keeps the room: one toolbar row and the status bar.
    let viewport = h.app.editor.viewport.get();
    assert!(
        viewport.height > H as f32 - row_height - 40.0,
        "{viewport:?}, toolbar row {row_height}"
    );

    // A swipe on the toolbar scrolls it and presses nothing.
    let middle = row_height / 2.0;
    h.swipe((300.0, middle), (40.0, middle));
    assert_eq!(h.app.editor.mode, crate::Mode::Wysiwyg);
    assert_eq!(h.app.editor.markdown(), "text\n");

    // Scrolled to the end (here with the wheel), the last button works.
    h.event(InputEvent::Scroll {
        position: kurbo::Point::new(width / 2.0, middle as f64),
        delta: frust::authoring::ScrollDelta::Pixels(0.0, 5000.0),
    });
    let (_, slots) = chrome::layout(&row, Point::new(width as f32 - content, 0.0), content, 0.0);
    let last = slots
        .iter()
        .rev()
        .find(|slot| matches!(slot.item, Item::Button(_)))
        .expect("a button");
    assert!(last.rect.x + last.rect.width <= width as f32);
    let at = last.rect.center();
    h.tap(at.x, at.y);
    assert_eq!(h.app.editor.mode, crate::Mode::Source);
}

#[test]
fn a_narrow_status_bar_keeps_the_zoom_control() {
    let width = 300.0;
    let h = Harness::sized(App::new("- [ ] task\n"), editor_view, width, H);
    let bar = toolbar::status_bar(&h.app.editor);
    assert!(chrome::natural_width(&bar, 0.0) > width as f32);
    let fitted = chrome::fit(&bar, width as f32, 0.0);
    assert!(chrome::natural_width(&fitted, 0.0) <= width as f32);
    assert!(
        fitted
            .items
            .iter()
            .any(|item| matches!(item, Item::Row { .. }))
    );
}
