//! The Frust widget: the page (with its own scrolling), toolbar, link bar,
//! status bar, context menu and link dialog.
//!
//! [`MarkdownEditor::view`] takes a snapshot of the [`EditorState`] (cheap:
//! the layout and the recorded document are shared) and the application's
//! handler for [`Action`]s. The retained [`EditorWidget`] draws the snapshot
//! and turns input into actions; clipboard requests of the editor are
//! carried out with Frust's event context.

use super::chrome::{self, Bar, Item, Menu, Slot, Status};
use super::dialog::{self, DialogView};
use super::menu::ContextMenu;
use super::toolbar;
use super::{Action, Command, EditorState, Effect, Inner, Link};
use crate::doc::Selection;
use crate::draw::{self, Op};
use crate::features::Features;
use crate::fonts;
use crate::geometry::{Point, Rectangle, Size, ToFrust as _, ToKurbo as _};
use crate::keyboard::{self, Key, Modifiers, Named, ScrollDelta};
use crate::layout::Layout;
use crate::platform;
use crate::render;
use crate::style::Palette;
use crate::zoom::ZoomSettings;
use frust::authoring::text::{FontFamily, TextStyle};
use frust::authoring::{
    BoxConstraints, BuildCtx, ChangeFlags, ChildPod, CursorIcon, EditCommand, EditingState,
    ErasedArgCallback, EventCtx, EventResult, FileDropPhase, ImeContentType, ImeEvent, ImeState,
    InputEvent, LayoutCtx, PaintCtx, PaintScene, PointerButton, PointerEvent, PointerPhase,
    SemanticsCtx, TypedArgCallback, View, Widget, build_child, erase_callback_arg, rebuild_child,
    route_event_single, teardown_child,
};
use frust::{TextInputView, text_input};
use frust_core::event::PointerSource;
use markdown_editor_core::editor::builtin_command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use web_time::Instant;

/// Width of the scrollbars.
const BAR: f32 = 10.0;
/// Pixels per wheel "line".
const LINE: f32 = 48.0;
/// Size used for a `Fill` dimension when the parent gives unbounded room.
const FALLBACK: Size = Size::new(800.0, 600.0);
/// A finger that moved less than this is still a tap or a long press.
const TOUCH_SLOP: f32 = 10.0;
/// A finger held still this long selects text instead of scrolling.
const LONG_PRESS: Duration = Duration::from_millis(500);
/// Two taps closer than this (in time and distance) are a double tap.
const MULTI_TAP: Duration = Duration::from_millis(450);
const MULTI_TAP_DISTANCE: f32 = 24.0;
/// Pinch zoom moves in steps of at least this ratio (each step lays out
/// the document again).
const PINCH_STEP: f32 = 0.03;
/// A toolbar that would wrap onto more lines than this becomes a single
/// row that scrolls sideways (phones).
const TOOLBAR_LINES: usize = 2;

/// Width or height of the editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// All the room the parent offers. Inside a Frust `column()` / `row()`,
    /// add the editor with `.flex(1, …)` so that room is bounded.
    Fill,
    /// A fixed number of logical pixels.
    Fixed(f32),
}

impl From<f32> for Length {
    fn from(pixels: f32) -> Self {
        Length::Fixed(pixels)
    }
}

impl Length {
    fn resolve(self, max: f64, fallback: f32) -> f32 {
        match self {
            Length::Fill if max.is_finite() => max as f32,
            Length::Fill => fallback,
            Length::Fixed(pixels) => pixels,
        }
    }
}

/// Builder returned by [`markdown_editor`].
pub struct MarkdownEditor<'a> {
    state: &'a EditorState,
    toolbar: bool,
    status_bar: bool,
    context_menu: bool,
    width: Length,
    height: Length,
}

/// Creates the editor widget for the given state.
///
/// ```no_run
/// use frust::{Component, View};
/// use frust_markdown_editor::{EditorState, Event, markdown_editor};
///
/// struct App;
///
/// struct State {
///     editor: EditorState,
/// }
///
/// impl Component for App {
///     type State = State;
///
///     fn init(&self) -> State {
///         State { editor: EditorState::with_markdown("# Hello") }
///     }
///
///     fn build(&self, state: &mut State) -> impl View<State> {
///         markdown_editor(&state.editor).view(|state: &mut State, action| {
///             let output = state.editor.update(action);
///             if output.events.contains(&Event::Changed) {
///                 let _markdown = state.editor.markdown();
///             }
///         })
///     }
/// }
/// ```
pub fn markdown_editor(state: &EditorState) -> MarkdownEditor<'_> {
    MarkdownEditor {
        state,
        toolbar: true,
        status_bar: true,
        context_menu: true,
        width: Length::Fill,
        height: Length::Fill,
    }
}

impl<'a> MarkdownEditor<'a> {
    /// Shows or hides the built-in toolbar (default: shown).
    pub fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// Shows or hides the status bar (default: shown).
    pub fn status_bar(mut self, show: bool) -> Self {
        self.status_bar = show;
        self
    }

    /// Enables or disables the context menu of the right mouse button
    /// (default: enabled).
    pub fn context_menu(mut self, enabled: bool) -> Self {
        self.context_menu = enabled;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Builds the Frust view. `on_action` receives every [`Action`] of the
    /// widget; pass it to [`EditorState::update`] and react to the returned
    /// [`Event`](super::Event)s.
    pub fn view<State: 'static>(
        self,
        on_action: impl Fn(&mut State, Action) + 'static,
    ) -> EditorView<State> {
        let state = self.state;
        state.sync_reported_size();
        let handler: TypedArgCallback<State, Action> = Rc::new(on_action);
        let palette = state.style.palette;

        let (scroll_x, scroll_y) = state.clamped_scroll();
        let page = Page {
            layout: state.layout.borrow().clone(),
            scene: state.content_scene(),
            palette,
            scroll: (scroll_x, scroll_y),
            viewport: state.viewport.get(),
            sel: state.sel,
            focused: state.focused,
            link_open: state.link_url.is_some(),
            last_activity: state.last_activity,
            zoom_settings: state.zoom_settings.clone(),
            tab_width: state.tab_width,
            features: state.features,
            accepts_drops: state.image_settings.enabled() && state.image_settings.drop,
            has_effects: !state.lock_link().effects.is_empty(),
            touch: state.touch,
        };

        let link_dialog = state
            .link_dialog
            .as_ref()
            .filter(|_| state.link_url.is_some());
        let bars = Bars {
            toolbar: self.toolbar.then(|| toolbar::toolbar(state)),
            link: (state.link_url.is_some() && link_dialog.is_none())
                .then(|| toolbar::link_bar(&palette, state.language, state.touch)),
            status: self.status_bar.then(|| toolbar::status_bar(state)),
            dialog: link_dialog.map(|dialog| dialog::view(dialog, &palette, state.language)),
            context: state
                .context_menu
                .filter(|_| self.context_menu)
                .map(|at| ContextMenu {
                    at,
                    entries: super::menu::entries(state),
                    touch: state.touch,
                }),
        };

        // A text field of the link bar or dialog.
        let field = |value: String, on_change: fn(String) -> Inner| {
            let changed = handler.clone();
            let submitted = handler.clone();
            let mut style = TextStyle::new(13.0, palette.ink.to_frust());
            style.family = FontFamily::named(fonts::SANS_FAMILY);
            text_input(value, move |app: &mut State, value: String| {
                changed(app, Action(on_change(value)));
            })
            .on_submit(move |app: &mut State, _| submitted(app, Action(Inner::LinkApply)))
            .text_style(style)
            .padding(6.0, 6.0)
        };
        let link_input = state
            .link_url
            .as_ref()
            .map(|url| field(url.clone(), Inner::LinkUrl).placeholder("https://"));
        let text_input = link_dialog
            .and_then(|dialog| dialog.text.clone())
            .map(|text| field(text, Inner::LinkText));

        EditorView {
            page,
            bars,
            link_input,
            text_input,
            context_menu: self.context_menu,
            handler,
            link: state.link.clone(),
            width: self.width,
            height: self.height,
        }
    }
}

/// What the widget needs from the state to draw the page and handle input.
#[derive(Clone)]
struct Page {
    layout: Arc<Layout>,
    scene: Arc<Vec<Op>>,
    palette: Palette,
    scroll: (f32, f32),
    /// The page size the layout was computed for.
    viewport: Size,
    sel: Selection,
    focused: bool,
    link_open: bool,
    last_activity: Instant,
    zoom_settings: ZoomSettings,
    tab_width: usize,
    features: Features,
    accepts_drops: bool,
    /// Effects (e.g. a copy run by the application) wait for the widget.
    has_effects: bool,
    /// The last pointer input was a touch.
    touch: bool,
}

impl PartialEq for Page {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.layout, &other.layout)
            && Arc::ptr_eq(&self.scene, &other.scene)
            && self.palette == other.palette
            && self.scroll == other.scroll
            && self.viewport == other.viewport
            && self.sel == other.sel
            && self.focused == other.focused
            && self.link_open == other.link_open
            && self.last_activity == other.last_activity
            && self.zoom_settings == other.zoom_settings
            && self.tab_width == other.tab_width
            && self.features == other.features
            && self.accepts_drops == other.accepts_drops
            && self.touch == other.touch
    }
}

#[derive(Clone, PartialEq)]
struct Bars {
    toolbar: Option<Bar>,
    link: Option<Bar>,
    status: Option<Bar>,
    /// The open link dialog (its address field is `link_input`).
    dialog: Option<DialogView>,
    context: Option<ContextMenu>,
}

/// The view returned by [`MarkdownEditor::view`].
pub struct EditorView<State: 'static> {
    page: Page,
    bars: Bars,
    /// The address field of the link bar or dialog.
    link_input: Option<TextInputView<State>>,
    /// The "text to display" field of the link dialog.
    text_input: Option<TextInputView<State>>,
    context_menu: bool,
    handler: TypedArgCallback<State, Action>,
    link: Arc<Mutex<Link>>,
    width: Length,
    height: Length,
}

/// Which bar a slot belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarId {
    Toolbar,
    Link,
    Status,
    /// The buttons of the link dialog.
    Dialog,
}

/// A slot of a bar.
type SlotRef = (BarId, usize);

/// What the primary button is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    /// Selecting text.
    Select,
    /// Dragging the vertical scrollbar; offset of the grab in the thumb.
    Vertical(f32),
    /// Dragging the horizontal scrollbar.
    Horizontal(f32),
    /// A bar item is pressed.
    Slot(SlotRef),
    /// A press on a toolbar that scrolls sideways: a tap on `slot` (if
    /// any), or a pan once it moved. `start` and `last` are x positions.
    Toolbar {
        slot: Option<SlotRef>,
        start: f32,
        last: f32,
        panning: bool,
    },
    /// A finger on the page.
    Touch(TouchDrag),
}

/// What a finger on the page does.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesture {
    /// Not decided yet: a tap, a pan or (held still) a long press.
    Pending,
    /// Scrolls the page.
    Pan,
    /// Selects text (moved after a long press).
    Select,
    /// Two fingers zoom; their distance at the last zoom step.
    Pinch(f32),
}

/// A touch gesture on the page.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TouchDrag {
    /// Where the finger went down and where it is now (widget coordinates).
    start: Point,
    last: Point,
    /// Where it went down in page coordinates. Kept, as the page can move
    /// before the finger is lifted (the bars grow for touch).
    local: Point,
    since: Instant,
    gesture: Gesture,
}

fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// An open drop-down list.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OpenMenu {
    slot: SlotRef,
    first: usize,
    hovered: Option<usize>,
}

/// Work for the next `Housekeeping` broadcast (where the application state
/// can be reached outside of user input).
#[derive(Debug, Clone, Copy, Default)]
struct Pending {
    /// Frust moved the focus elsewhere: tell the state.
    blur: bool,
    /// The state wants the focus: claim it.
    claim: bool,
    /// Focus arrived from Frust (e.g. Tab traversal): tell the state.
    focus: bool,
    /// The link dialog opened: focus its address field.
    dialog_focus: bool,
}

/// The retained widget of an [`EditorView`].
pub struct EditorWidget {
    page: Page,
    bars: Bars,
    handler: ErasedArgCallback<Action>,
    link: Arc<Mutex<Link>>,
    width: Length,
    height: Length,
    context_menu: bool,
    link_pod: Option<ChildPod>,
    text_pod: Option<ChildPod>,

    // Layout.
    size: Size,
    page_rect: Rectangle,
    toolbar: (Rectangle, Vec<Slot>),
    /// How far a toolbar too wide for one row is scrolled sideways, and
    /// how far it can be (`0`: the toolbar wraps instead).
    toolbar_scroll: f32,
    toolbar_max: f32,
    link_bar: (Rectangle, Vec<Slot>),
    status: (Rectangle, Vec<Slot>),
    dialog: Option<dialog::Layout>,

    // Interaction.
    drag: Drag,
    last_click: Option<(Instant, Point)>,
    clicks: u8,
    modifiers: Modifiers,
    hovered: Option<SlotRef>,
    hover_point: Option<Point>,
    menu: Option<OpenMenu>,
    /// The context menu entry under the pointer.
    context_hover: Option<usize>,
    /// A second finger on the page (pinch zoom).
    second_touch: Option<Point>,

    // Focus.
    had_focus: bool,
    pending: Pending,
    last_claim: Option<Instant>,
}

impl<State: 'static> View<State> for EditorView<State> {
    type Element = EditorWidget;

    fn build(&self, ctx: &mut BuildCtx<'_>) -> EditorWidget {
        let link_pod = self
            .link_input
            .as_ref()
            .map(|input| build_child(input, ctx));
        let text_pod = self
            .text_input
            .as_ref()
            .map(|input| build_child(input, ctx));
        let mut widget = EditorWidget {
            page: self.page.clone(),
            bars: self.bars.clone(),
            handler: erase_callback_arg(&self.handler),
            link: self.link.clone(),
            width: self.width,
            height: self.height,
            context_menu: self.context_menu,
            link_pod,
            text_pod,
            size: Size::ZERO,
            page_rect: Rectangle::default(),
            toolbar: (Rectangle::default(), Vec::new()),
            toolbar_scroll: 0.0,
            toolbar_max: 0.0,
            link_bar: (Rectangle::default(), Vec::new()),
            status: (Rectangle::default(), Vec::new()),
            dialog: None,
            drag: Drag::None,
            last_click: None,
            clicks: 0,
            modifiers: Modifiers::empty(),
            hovered: None,
            hover_point: None,
            menu: None,
            context_hover: None,
            second_touch: None,
            had_focus: false,
            pending: Pending {
                dialog_focus: self.bars.dialog.is_some(),
                ..Pending::default()
            },
            last_claim: None,
        };
        widget.sync(ctx.has_focus());
        widget
    }

    fn rebuild(
        &self,
        prev: &Self,
        element: &mut EditorWidget,
        ctx: &mut BuildCtx<'_>,
    ) -> ChangeFlags {
        let mut flags = ChangeFlags::NONE;
        element.handler = erase_callback_arg(&self.handler);
        element.link = self.link.clone();
        element.context_menu = self.context_menu;

        flags |= rebuild_field(
            &prev.link_input,
            &self.link_input,
            &mut element.link_pod,
            ctx,
        );
        flags |= rebuild_field(
            &prev.text_input,
            &self.text_input,
            &mut element.text_pod,
            ctx,
        );

        if self.bars != element.bars || self.width != element.width || self.height != element.height
        {
            if element.bars.dialog.is_none() && self.bars.dialog.is_some() {
                element.pending.dialog_focus = true;
            }
            element.bars = self.bars.clone();
            element.width = self.width;
            element.height = self.height;
            element.menu = None;
            element.context_hover = None;
            flags |= ChangeFlags::LAYOUT | ChangeFlags::PAINT;
        }
        if self.page != element.page {
            flags |= ChangeFlags::PAINT;
        }
        element.page = self.page.clone();
        element.sync(ctx.has_focus());
        flags
    }

    fn teardown(&self, element: &mut EditorWidget, ctx: &mut BuildCtx<'_>) {
        if let (Some(input), Some(pod)) = (&self.link_input, element.link_pod.as_mut()) {
            teardown_child(input, pod, ctx);
        }
        if let (Some(input), Some(pod)) = (&self.text_input, element.text_pod.as_mut()) {
            teardown_child(input, pod, ctx);
        }
    }
}

/// Builds, updates or removes the pod of an optional text field.
fn rebuild_field<State: 'static>(
    previous: &Option<TextInputView<State>>,
    next: &Option<TextInputView<State>>,
    pod: &mut Option<ChildPod>,
    ctx: &mut BuildCtx<'_>,
) -> ChangeFlags {
    match (previous, next, pod.take()) {
        (Some(previous), Some(next), Some(mut existing)) => {
            let flags = rebuild_child(previous, next, &mut existing, ctx);
            *pod = Some(existing);
            flags
        }
        (Some(previous), None, Some(mut existing)) => {
            // The focus stays with the editor, not with the whole session.
            existing.set_focused(false);
            teardown_child(previous, &mut existing, ctx);
            ChangeFlags::LAYOUT | ChangeFlags::PAINT
        }
        (_, Some(next), _) => {
            *pod = Some(build_child(next, ctx));
            ChangeFlags::LAYOUT | ChangeFlags::PAINT
        }
        (_, None, _) => ChangeFlags::NONE,
    }
}

/// Scrollbar geometry: track and thumb, in page coordinates.
#[derive(Debug, Clone, Copy)]
struct Scrollbar {
    track: Rectangle,
    thumb: Rectangle,
    /// Largest scroll offset.
    max: f32,
}

/// The vertical and horizontal scrollbar, when the content overflows.
fn scrollbars(page: &Page, size: Size) -> (Option<Scrollbar>, Option<Scrollbar>) {
    let (content_w, content_h) = (page.layout.width, page.layout.height);
    let (sx, sy) = page.scroll;
    let need_v = content_h > size.height + 0.5;
    let need_h = content_w > size.width + 0.5;

    let vertical = need_v.then(|| {
        let length = size.height - if need_h { BAR } else { 0.0 };
        let max = (content_h - size.height).max(1.0);
        let thumb = (length * size.height / content_h).clamp(24.0, length.max(24.0));
        let y = (sy / max).clamp(0.0, 1.0) * (length - thumb);
        Scrollbar {
            track: Rectangle::new(Point::new(size.width - BAR, 0.0), Size::new(BAR, length)),
            thumb: Rectangle::new(Point::new(size.width - BAR, y), Size::new(BAR, thumb)),
            max,
        }
    });
    let horizontal = need_h.then(|| {
        let length = size.width - if need_v { BAR } else { 0.0 };
        let max = (content_w - size.width).max(1.0);
        let thumb = (length * size.width / content_w).clamp(24.0, length.max(24.0));
        let x = (sx / max).clamp(0.0, 1.0) * (length - thumb);
        Scrollbar {
            track: Rectangle::new(Point::new(0.0, size.height - BAR), Size::new(length, BAR)),
            thumb: Rectangle::new(Point::new(x, size.height - BAR), Size::new(thumb, BAR)),
            max,
        }
    });
    (vertical, horizontal)
}

/// Scroll offset for a thumb start position.
fn scroll_for(thumb_start: f32, track_start: f32, track: f32, thumb: f32, max: f32) -> f32 {
    let room = (track - thumb).max(1.0);
    ((thumb_start - track_start) / room).clamp(0.0, 1.0) * max
}

/// The text input session of the focused page: it lets the platform show
/// its on-screen keyboard and input method, at `caret` (window
/// coordinates).
fn ime_state(caret: Option<frust::kurbo::Rect>) -> ImeState {
    ImeState {
        active: true,
        editing: EditingState::default(),
        caret,
        content_type: ImeContentType::Normal,
        suppress_soft_keyboard: false,
    }
}

fn point(p: frust::kurbo::Point) -> Point {
    Point::new(p.x as f32, p.y as f32)
}

impl EditorWidget {
    // ----------------------------------------------------------------
    // State plumbing
    // ----------------------------------------------------------------

    /// Hands an action to the application, then performs the effects the
    /// editor asked for.
    fn dispatch(&mut self, ctx: &mut EventCtx, inner: Inner) {
        (self.handler)(ctx, Action(inner));
        self.perform_effects(ctx);
        ctx.request_redraw();
    }

    fn perform_effects(&mut self, ctx: &mut EventCtx) {
        let effects = {
            let mut link = self.link.lock().unwrap_or_else(|p| p.into_inner());
            std::mem::take(&mut link.effects)
        };
        for effect in effects {
            match effect {
                Effect::WriteClipboard(text) => ctx.write_clipboard(text),
                Effect::ReadClipboard => ctx.request_paste(),
                // The widget focuses and cycles the dialog's fields itself.
                Effect::FocusLinkDialog | Effect::CycleLinkFocus => {}
            }
        }
    }

    /// The text field (link bar or dialog) holding the keyboard focus.
    fn focused_field(&mut self) -> Option<&mut ChildPod> {
        [self.link_pod.as_mut(), self.text_pod.as_mut()]
            .into_iter()
            .flatten()
            .find(|pod| pod.holds_live_focus())
    }

    /// A text field (link bar or dialog) holds the keyboard focus.
    fn input_focused(&self) -> bool {
        [self.link_pod.as_ref(), self.text_pod.as_ref()]
            .into_iter()
            .flatten()
            .any(ChildPod::holds_live_focus)
    }

    /// Gives the keyboard focus to a field of the link dialog (`text`: the
    /// text to display, otherwise the address) and selects its content.
    fn focus_field(&mut self, ctx: &mut EventCtx, text: bool) {
        let (target, other) = if text {
            (&mut self.text_pod, &mut self.link_pod)
        } else {
            (&mut self.link_pod, &mut self.text_pod)
        };
        let Some(target) = target.as_mut() else {
            return;
        };
        if let Some(other) = other.as_mut() {
            other.set_focused(false);
        }
        target.set_focused(true);
        ctx.request_focus();
        target.event_child(ctx, &InputEvent::EditCommand(EditCommand::SelectAll));
        ctx.request_redraw();
    }

    /// Compares Frust's focus (`chain`: the focus path reaches this widget)
    /// with the state and schedules what is needed to match them.
    fn sync(&mut self, chain: bool) {
        let lost = self.had_focus && !chain;
        let gained = !self.had_focus && chain;
        self.had_focus = chain;

        if lost && self.page.focused {
            self.pending.blur = true;
        } else if gained && !self.page.focused && !self.page.link_open && !self.input_focused() {
            self.pending.focus = true;
        } else if !chain && self.page.focused {
            let recent = self
                .last_claim
                .is_some_and(|at| at.elapsed() < Duration::from_millis(250));
            if !recent {
                self.pending.claim = true;
            }
        }
        let pending = self.pending;
        if pending.blur
            || pending.claim
            || pending.focus
            || pending.dialog_focus
            || self.page.has_effects
        {
            frust_core::mark_pending_result_flush();
        }
    }

    fn housekeeping(&mut self, ctx: &mut EventCtx) {
        let pending = std::mem::take(&mut self.pending);
        if pending.blur {
            self.dispatch(ctx, Inner::PressOutside);
        }
        if pending.focus {
            self.dispatch(ctx, Inner::Command(Command::Focus));
        }
        if pending.claim {
            self.last_claim = Some(Instant::now());
            ctx.request_focus();
            ctx.request_redraw();
        }
        if pending.dialog_focus {
            self.focus_field(ctx, false);
        }
        self.perform_effects(ctx);
    }

    // ----------------------------------------------------------------
    // Geometry
    // ----------------------------------------------------------------

    fn slots(&self, bar: BarId) -> &[Slot] {
        match bar {
            BarId::Toolbar => &self.toolbar.1,
            BarId::Link => &self.link_bar.1,
            BarId::Status => &self.status.1,
            BarId::Dialog => self
                .dialog
                .as_ref()
                .map_or(&[][..], |dialog| &dialog.buttons.1),
        }
    }

    fn slot(&self, slot: SlotRef) -> Option<&Slot> {
        self.slots(slot.0).get(slot.1)
    }

    /// The interactive bar item under `point` (widget coordinates).
    fn slot_at(&self, point: Point) -> Option<SlotRef> {
        // The dialog is modal: only its buttons react.
        let bars: &[BarId] = if self.bars.dialog.is_some() {
            &[BarId::Dialog]
        } else {
            &[BarId::Toolbar, BarId::Link, BarId::Status]
        };
        bars.iter().copied().find_map(|bar| {
            self.slots(bar)
                .iter()
                .position(|slot| slot.interactive() && slot.rect.contains(point))
                .map(|index| (bar, index))
        })
    }

    /// The open menu's geometry and its drop-down.
    fn open_menu(&self) -> Option<(chrome::Dropdown, Menu)> {
        let open = self.menu?;
        let slot = self.slot(open.slot)?;
        let Item::Dropdown(dropdown) = &slot.item else {
            return None;
        };
        let bounds = Rectangle::with_size(self.size);
        let menu = chrome::menu(dropdown, slot.rect, bounds, open.first);
        Some((dropdown.clone(), menu))
    }

    /// `point` in page coordinates, if inside the page.
    fn in_page(&self, point: Point) -> Option<Point> {
        self.page_rect
            .contains(point)
            .then(|| Point::new(point.x - self.page_rect.x, point.y - self.page_rect.y))
    }

    /// Page coordinates to document coordinates.
    fn to_document(&self, point: Point) -> (f32, f32) {
        let (sx, sy) = self.page.scroll;
        (point.x + sx, point.y + sy)
    }

    fn current_modifiers(&mut self) -> Modifiers {
        if let Some(modifiers) = platform::modifiers() {
            self.modifiers = modifiers;
        }
        self.modifiers
    }

    // ----------------------------------------------------------------
    // Pointer
    // ----------------------------------------------------------------

    fn pointer(&mut self, ctx: &mut EventCtx, event: &InputEvent, p: &PointerEvent) -> EventResult {
        let at = point(p.position);

        // The link fields get their own gestures.
        let down = p.phase == PointerPhase::Down;
        let hit = |pod: &Option<ChildPod>| {
            pod.as_ref()
                .is_some_and(|pod| pod.is_active() || (down && pod.contains(p.position)))
        };
        let on_link = hit(&self.link_pod);
        if on_link || hit(&self.text_pod) {
            let (target, other) = if on_link {
                (&mut self.link_pod, &mut self.text_pod)
            } else {
                (&mut self.text_pod, &mut self.link_pod)
            };
            if down && let Some(other) = other.as_mut() {
                other.set_focused(false);
            }
            self.menu = None;
            if let Some(pod) = target.as_mut() {
                return route_event_single(pod, ctx, event);
            }
        }

        let contact = ctx.pointer_id();
        let touch = contact.source == PointerSource::Touch;
        if touch && contact.slot > 0 {
            return self.second_finger(ctx, at, p.phase);
        }
        if down && touch != self.page.touch {
            // Larger targets for fingers, the usual ones for a mouse.
            self.dispatch(ctx, Inner::Touch(touch));
        }

        match p.phase {
            PointerPhase::Down => self.pointer_down(ctx, at, p.button, touch),
            PointerPhase::Move => self.pointer_move(ctx, at),
            PointerPhase::Up => {
                let result = self.pointer_up(ctx, at);
                if touch {
                    // No hover without a mouse: nothing stays highlighted.
                    self.hovered = None;
                    self.hover_point = None;
                }
                result
            }
            PointerPhase::Cancel => {
                let active = self.drag != Drag::None;
                self.drag = Drag::None;
                self.second_touch = None;
                if active {
                    ctx.request_redraw();
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
        }
    }

    fn pointer_down(
        &mut self,
        ctx: &mut EventCtx,
        at: Point,
        button: PointerButton,
        touch: bool,
    ) -> EventResult {
        // The context menu takes a click on one of its entries; any other
        // press closes it and acts as usual.
        if let Some(context) = &self.bars.context {
            let action = context
                .item_at(self.size, at)
                .and_then(|index| context.action(index));
            if let Some(action) = action {
                ctx.request_focus();
                self.dispatch(ctx, action.0);
                return EventResult::Handled;
            }
            self.dispatch(ctx, Inner::CloseMenu);
        }

        // An open drop-down list takes the click.
        if let Some((dropdown, menu)) = self.open_menu() {
            let open = self.menu.take();
            ctx.request_redraw();
            if let Some(index) = menu.option_at(&dropdown, at) {
                if self.page.focused {
                    ctx.request_focus();
                }
                let action = dropdown.options[index].1.clone();
                self.dispatch(ctx, action.0);
                return EventResult::Handled;
            }
            if open.is_some_and(|open| self.slot(open.slot).is_some_and(|s| s.rect.contains(at))) {
                // A click on the drop-down itself closes it.
                if self.page.focused {
                    ctx.request_focus();
                }
                return EventResult::Handled;
            }
        }

        // The link dialog is modal: only its buttons and fields react.
        if self.bars.dialog.is_some() {
            if button == PointerButton::Primary
                && let Some(slot) = self.slot_at(at)
            {
                self.drag = Drag::Slot(slot);
                ctx.capture_pointer();
                ctx.request_redraw();
            }
            return EventResult::Handled;
        }

        if button != PointerButton::Primary {
            let local = self.in_page(at);
            let on_scrollbar = local.is_some_and(|local| {
                let (vertical, horizontal) = scrollbars(&self.page, self.page_rect.size());
                vertical.is_some_and(|bar| bar.track.contains(local))
                    || horizontal.is_some_and(|bar| bar.track.contains(local))
            });
            if let Some(local) = local.filter(|_| !on_scrollbar) {
                let (x, y) = self.to_document(local);
                if button == PointerButton::Middle {
                    self.dispatch(ctx, Inner::OpenLink { x, y });
                    return EventResult::Handled;
                }
                if button == PointerButton::Secondary && self.context_menu {
                    ctx.request_focus();
                    self.dispatch(ctx, Inner::ContextMenu { at, x, y });
                    return EventResult::Handled;
                }
            }
            return EventResult::Ignored;
        }

        // A toolbar that scrolls sideways: a tap on a button or a pan,
        // decided when the pointer moves or is released.
        if self.toolbar_max > 0.0 && self.toolbar.0.contains(at) {
            if self.page.focused {
                ctx.request_focus();
            }
            let slot = self.slot_at(at).filter(|slot| slot.0 == BarId::Toolbar);
            self.hovered = slot;
            self.drag = Drag::Toolbar {
                slot,
                start: at.x,
                last: at.x,
                panning: false,
            };
            ctx.capture_pointer();
            ctx.request_redraw();
            return EventResult::Handled;
        }

        // Bars.
        if let Some(slot) = self.slot_at(at) {
            if self.page.focused {
                // Keep typing in the page after using the toolbar.
                ctx.request_focus();
            }
            if matches!(self.slot(slot).map(|s| &s.item), Some(Item::Dropdown(_))) {
                self.menu = Some(OpenMenu {
                    slot,
                    first: 0,
                    hovered: None,
                });
                ctx.request_redraw();
                return EventResult::Handled;
            }
            self.drag = Drag::Slot(slot);
            ctx.capture_pointer();
            ctx.request_redraw();
            return EventResult::Handled;
        }

        let Some(local) = self.in_page(at) else {
            // Bar background: keep the focus where it is.
            let on_bar = self.toolbar.0.contains(at)
                || self.link_bar.0.contains(at)
                || self.status.0.contains(at);
            if self.page.focused && on_bar {
                ctx.request_focus();
            }
            return EventResult::Ignored;
        };
        let size = self.page_rect.size();

        // Scrollbars. A finger near the edge pans the page unless it is
        // on a thumb.
        let (vertical, horizontal) = scrollbars(&self.page, size);
        let grabs = |bar: &Scrollbar| {
            if touch {
                bar.thumb.contains(local)
            } else {
                bar.track.contains(local)
            }
        };
        let (vertical, horizontal) = (vertical.filter(grabs), horizontal.filter(grabs));
        if let Some(bar) = vertical {
            let grab = if bar.thumb.contains(local) {
                local.y - bar.thumb.y
            } else {
                bar.thumb.height / 2.0
            };
            self.drag = Drag::Vertical(grab);
            ctx.capture_pointer();
            if self.page.focused {
                ctx.request_focus();
            }
            let y = scroll_for(
                local.y - grab,
                bar.track.y,
                bar.track.height,
                bar.thumb.height,
                bar.max,
            );
            self.dispatch(
                ctx,
                Inner::ScrollTo {
                    x: None,
                    y: Some(y),
                },
            );
            return EventResult::Handled;
        }
        if let Some(bar) = horizontal {
            let grab = if bar.thumb.contains(local) {
                local.x - bar.thumb.x
            } else {
                bar.thumb.width / 2.0
            };
            self.drag = Drag::Horizontal(grab);
            ctx.capture_pointer();
            if self.page.focused {
                ctx.request_focus();
            }
            let x = scroll_for(
                local.x - grab,
                bar.track.x,
                bar.track.width,
                bar.thumb.width,
                bar.max,
            );
            self.dispatch(
                ctx,
                Inner::ScrollTo {
                    x: Some(x),
                    y: None,
                },
            );
            return EventResult::Handled;
        }

        // A finger: a tap, a pan, a long press or a pinch, decided by what
        // it does next.
        if touch {
            self.drag = Drag::Touch(TouchDrag {
                start: at,
                last: at,
                local,
                since: Instant::now(),
                gesture: Gesture::Pending,
            });
            ctx.capture_pointer();
            // A second finger reaches this widget too (pinch).
            ctx.capture_contacts();
            return EventResult::Handled;
        }

        // Text.
        let now = Instant::now();
        let clicks = match self.last_click {
            Some((time, previous))
                if now.duration_since(time) < Duration::from_millis(450)
                    && (previous.x - local.x).abs() < 5.0
                    && (previous.y - local.y).abs() < 5.0 =>
            {
                self.clicks % 3 + 1
            }
            _ => 1,
        };
        self.clicks = clicks;
        self.last_click = Some((now, local));
        let (x, y) = self.to_document(local);
        self.drag = if self.page.layout.task_at(x, y).is_some() {
            Drag::None
        } else {
            Drag::Select
        };

        ctx.request_focus();
        ctx.capture_pointer();
        let modifiers = self.current_modifiers();
        self.dispatch(ctx, Inner::Modifiers(modifiers));
        self.dispatch(ctx, Inner::Press { x, y, clicks });
        EventResult::Handled
    }

    fn pointer_move(&mut self, ctx: &mut EventCtx, at: Point) -> EventResult {
        match self.drag {
            Drag::Select => {
                let local = Point::new(at.x - self.page_rect.x, at.y - self.page_rect.y);
                let (x, y) = self.to_document(local);
                self.dispatch(ctx, Inner::Drag { x, y });
                ctx.set_cursor(CursorIcon::Text);
                return EventResult::Handled;
            }
            Drag::Vertical(grab) => {
                let local = Point::new(at.x - self.page_rect.x, at.y - self.page_rect.y);
                if let Some(bar) = scrollbars(&self.page, self.page_rect.size()).0 {
                    let y = scroll_for(
                        local.y - grab,
                        bar.track.y,
                        bar.track.height,
                        bar.thumb.height,
                        bar.max,
                    );
                    self.dispatch(
                        ctx,
                        Inner::ScrollTo {
                            x: None,
                            y: Some(y),
                        },
                    );
                }
                return EventResult::Handled;
            }
            Drag::Horizontal(grab) => {
                let local = Point::new(at.x - self.page_rect.x, at.y - self.page_rect.y);
                if let Some(bar) = scrollbars(&self.page, self.page_rect.size()).1 {
                    let x = scroll_for(
                        local.x - grab,
                        bar.track.x,
                        bar.track.width,
                        bar.thumb.width,
                        bar.max,
                    );
                    self.dispatch(
                        ctx,
                        Inner::ScrollTo {
                            x: Some(x),
                            y: None,
                        },
                    );
                }
                return EventResult::Handled;
            }
            Drag::Slot(slot) => {
                let inside = self.slot(slot).is_some_and(|s| s.rect.contains(at));
                let hovered = inside.then_some(slot);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_redraw();
                }
                ctx.set_cursor(CursorIcon::Pointer);
                return EventResult::Handled;
            }
            Drag::Toolbar {
                slot,
                start,
                last,
                panning,
            } => {
                let panning = panning || (at.x - start).abs() >= TOUCH_SLOP;
                if panning && self.scroll_toolbar(last - at.x) {
                    ctx.request_redraw();
                }
                let hovered = slot.filter(|&slot| {
                    !panning && self.slot(slot).is_some_and(|s| s.rect.contains(at))
                });
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_redraw();
                }
                self.drag = Drag::Toolbar {
                    slot,
                    start,
                    last: at.x,
                    panning,
                };
                return EventResult::Handled;
            }
            Drag::Touch(gesture) => {
                self.touch_move(ctx, at, gesture);
                return EventResult::Handled;
            }
            Drag::None => {}
        }

        // Hover.
        ctx.claim_hover();
        if let Some(context) = &self.bars.context {
            let hovered = context.item_at(self.size, at);
            if hovered != self.context_hover {
                self.context_hover = hovered;
                ctx.request_redraw();
            }
            if context.rect(self.size).contains(at) {
                ctx.set_cursor(if hovered.is_some() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                });
                return EventResult::Handled;
            }
        }
        if self.bars.dialog.is_some() {
            let hovered = self.slot_at(at);
            if hovered != self.hovered {
                self.hovered = hovered;
                ctx.request_redraw();
            }
            self.hover_point = None;
            ctx.set_cursor(if hovered.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            });
            return EventResult::Handled;
        }
        let mut changed = false;
        if let Some((dropdown, menu)) = self.open_menu() {
            let hovered = menu.option_at(&dropdown, at);
            if let Some(open) = self.menu.as_mut()
                && open.hovered != hovered
            {
                open.hovered = hovered;
                changed = true;
            }
            if hovered.is_some() {
                ctx.set_cursor(CursorIcon::Pointer);
                if changed {
                    ctx.request_redraw();
                }
                return EventResult::Handled;
            }
        }

        let hovered = self.slot_at(at);
        if hovered != self.hovered {
            self.hovered = hovered;
            changed = true;
        }
        let previous = self.hover_point;
        self.hover_point = self.in_page(at);
        let size = self.page_rect.size();
        let on_scrollbar = |local: Option<Point>| {
            let (vertical, horizontal) = scrollbars(&self.page, size);
            local.is_some_and(|local| {
                vertical.is_some_and(|bar| bar.track.contains(local))
                    || horizontal.is_some_and(|bar| bar.track.contains(local))
            })
        };
        if on_scrollbar(previous) != on_scrollbar(self.hover_point) {
            changed = true;
        }
        if changed {
            ctx.request_redraw();
        }

        if hovered.is_some() {
            ctx.set_cursor(CursorIcon::Pointer);
        } else if let Some(local) = self.hover_point {
            if on_scrollbar(Some(local)) {
                ctx.set_cursor(CursorIcon::Default);
            } else {
                let (x, y) = self.to_document(local);
                ctx.set_cursor(if self.page.layout.task_at(x, y).is_some() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Text
                });
            }
        }
        EventResult::Ignored
    }

    fn pointer_up(&mut self, ctx: &mut EventCtx, at: Point) -> EventResult {
        match std::mem::take(&mut self.drag) {
            Drag::None => EventResult::Ignored,
            Drag::Slot(slot) => {
                let action = self.slot(slot).and_then(|s| match &s.item {
                    Item::Button(button) if s.rect.contains(at) => button.action.clone(),
                    _ => None,
                });
                if let Some(action) = action {
                    self.dispatch(ctx, action.0);
                }
                ctx.request_redraw();
                EventResult::Handled
            }
            Drag::Toolbar {
                slot: Some(slot),
                panning: false,
                ..
            } => {
                let item = self
                    .slot(slot)
                    .filter(|s| s.rect.contains(at))
                    .map(|s| s.item.clone());
                match item {
                    Some(Item::Button(button)) => {
                        if let Some(action) = button.action {
                            self.dispatch(ctx, action.0);
                        }
                    }
                    Some(Item::Dropdown(_)) => {
                        self.menu = Some(OpenMenu {
                            slot,
                            first: 0,
                            hovered: None,
                        });
                    }
                    _ => {}
                }
                ctx.request_redraw();
                EventResult::Handled
            }
            Drag::Touch(gesture) => {
                self.touch_up(ctx, gesture);
                EventResult::Handled
            }
            Drag::Toolbar { .. } | Drag::Select | Drag::Vertical(_) | Drag::Horizontal(_) => {
                ctx.request_redraw();
                EventResult::Handled
            }
        }
    }

    // ----------------------------------------------------------------
    // Touch
    // ----------------------------------------------------------------

    /// A finger moved on the page: once it left the slop it pans, or
    /// selects when it was held still first.
    fn touch_move(&mut self, ctx: &mut EventCtx, at: Point, mut drag: TouchDrag) {
        if drag.gesture == Gesture::Pending {
            if distance(at, drag.start) < TOUCH_SLOP {
                return;
            }
            drag.gesture = if drag.since.elapsed() >= LONG_PRESS {
                self.touch_press(ctx, drag.local, 1);
                Gesture::Select
            } else {
                Gesture::Pan
            };
        }
        let previous = drag.last;
        drag.last = at;
        match drag.gesture {
            Gesture::Pan => {
                // The content follows the finger.
                let (dx, dy) = (previous.x - at.x, previous.y - at.y);
                self.dispatch(ctx, Inner::ScrollBy { dx, dy });
            }
            Gesture::Select => {
                let local = Point::new(at.x - self.page_rect.x, at.y - self.page_rect.y);
                let (x, y) = self.to_document(local);
                self.dispatch(ctx, Inner::Drag { x, y });
            }
            Gesture::Pinch(_) => self.pinch_step(ctx, &mut drag),
            Gesture::Pending => {}
        }
        self.drag = Drag::Touch(drag);
    }

    /// The finger was lifted: a tap places the caret (a double tap selects
    /// a word), a long press selects the word and opens the context menu.
    fn touch_up(&mut self, ctx: &mut EventCtx, drag: TouchDrag) {
        self.second_touch = None;
        ctx.request_redraw();
        if drag.gesture != Gesture::Pending {
            return;
        }
        let local = drag.local;
        if drag.since.elapsed() >= LONG_PRESS {
            self.last_click = None;
            self.touch_press(ctx, local, 2);
            if self.context_menu {
                let (x, y) = self.to_document(local);
                self.dispatch(
                    ctx,
                    Inner::ContextMenu {
                        at: drag.start,
                        x,
                        y,
                    },
                );
            }
            return;
        }
        let now = Instant::now();
        let clicks = match self.last_click {
            Some((time, previous))
                if now.duration_since(time) < MULTI_TAP
                    && distance(previous, local) < MULTI_TAP_DISTANCE =>
            {
                self.clicks % 3 + 1
            }
            _ => 1,
        };
        self.clicks = clicks;
        self.last_click = Some((now, local));
        self.touch_press(ctx, local, clicks);
    }

    /// A press at `local` (page coordinates) made with a finger. Claims the
    /// keyboard focus, which opens the on-screen keyboard.
    fn touch_press(&mut self, ctx: &mut EventCtx, local: Point, clicks: u8) {
        let (x, y) = self.to_document(local);
        ctx.request_focus();
        // Published right away, inside the user's gesture: mobile browsers
        // only show their keyboard then. Paint follows with the caret.
        ctx.publish_ime_state(ime_state(None));
        let modifiers = self.current_modifiers();
        self.dispatch(ctx, Inner::Modifiers(modifiers));
        self.dispatch(ctx, Inner::Press { x, y, clicks });
    }

    /// Another finger while one is on the page: two fingers zoom; lifting
    /// one goes back to scrolling.
    fn second_finger(&mut self, ctx: &mut EventCtx, at: Point, phase: PointerPhase) -> EventResult {
        let Drag::Touch(mut drag) = self.drag else {
            return EventResult::Ignored;
        };
        match phase {
            PointerPhase::Down => {
                self.second_touch = Some(at);
                drag.gesture = Gesture::Pinch(distance(drag.last, at));
            }
            PointerPhase::Move => {
                self.second_touch = Some(at);
                self.pinch_step(ctx, &mut drag);
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                self.second_touch = None;
                if matches!(drag.gesture, Gesture::Pinch(_)) {
                    drag.gesture = Gesture::Pan;
                }
            }
        }
        self.drag = Drag::Touch(drag);
        EventResult::Handled
    }

    /// Zooms by how much the two fingers moved apart since the last step.
    fn pinch_step(&mut self, ctx: &mut EventCtx, drag: &mut TouchDrag) {
        let (Gesture::Pinch(base), Some(second)) = (drag.gesture, self.second_touch) else {
            return;
        };
        let now = distance(drag.last, second);
        if base < 1.0 || now < 1.0 {
            return;
        }
        let scale = now / base;
        if (scale - 1.0).abs() < PINCH_STEP {
            return;
        }
        drag.gesture = Gesture::Pinch(now);
        let settings = &self.page.zoom_settings;
        if settings.enabled && settings.mouse_wheel {
            // `Wheel` multiplies the zoom by exp(0.0025 × y).
            let y = scale.ln() / 0.0025;
            self.dispatch(ctx, Inner::Wheel(ScrollDelta::Pixels { x: 0.0, y }));
        }
    }

    /// Scrolls a toolbar that is too wide for its row; returns whether it
    /// moved. The slots move along, so no new layout is needed.
    fn scroll_toolbar(&mut self, delta: f32) -> bool {
        let next = (self.toolbar_scroll + delta).clamp(0.0, self.toolbar_max);
        let moved = next - self.toolbar_scroll;
        if moved.abs() < 0.01 {
            return false;
        }
        self.toolbar_scroll = next;
        for slot in &mut self.toolbar.1 {
            slot.rect.x -= moved;
        }
        // Its drop-down list would be left behind.
        self.menu = None;
        true
    }

    // ----------------------------------------------------------------
    // Wheel, pinch, files
    // ----------------------------------------------------------------

    fn scroll(
        &mut self,
        ctx: &mut EventCtx,
        at: Point,
        delta: frust::authoring::ScrollDelta,
    ) -> EventResult {
        if self.bars.dialog.is_some() {
            return EventResult::Handled;
        }
        if let Some((dropdown, menu)) = self.open_menu()
            && menu.area.contains(at)
        {
            let lines = match delta {
                frust::authoring::ScrollDelta::Lines(_, y) => y,
                frust::authoring::ScrollDelta::Pixels(_, y) => y / 20.0,
            };
            if let Some(open) = self.menu.as_mut() {
                let max = dropdown.options.len().saturating_sub(menu.visible);
                let first = (open.first as f64 + lines.signum() * lines.abs().ceil()).max(0.0);
                open.first = (first as usize).min(max);
            }
            ctx.request_redraw();
            return EventResult::Handled;
        }
        if self.toolbar_max > 0.0 && self.toolbar.0.contains(at) {
            let delta = match delta {
                frust::authoring::ScrollDelta::Lines(x, y) => (if x != 0.0 { x } else { y }) * 40.0,
                frust::authoring::ScrollDelta::Pixels(x, y) => {
                    if x != 0.0 {
                        x
                    } else {
                        y
                    }
                }
            };
            if self.scroll_toolbar(delta as f32) {
                ctx.request_redraw();
            }
            return EventResult::Handled;
        }
        if self.in_page(at).is_none() {
            return EventResult::Ignored;
        }
        // Frust's deltas grow towards later content (down / right).
        let (mut dx, mut dy) = match delta {
            frust::authoring::ScrollDelta::Lines(x, y) => (x as f32 * LINE, y as f32 * LINE),
            frust::authoring::ScrollDelta::Pixels(x, y) => (x as f32, y as f32),
        };
        if dx == 0.0 && self.current_modifiers().shift() {
            std::mem::swap(&mut dx, &mut dy);
        }
        self.dispatch(ctx, Inner::ScrollBy { dx, dy });
        EventResult::Handled
    }

    /// Ctrl/Cmd + wheel and touchpad pinch.
    fn pinch(&mut self, ctx: &mut EventCtx, at: Point, scale: f64) -> EventResult {
        if self.bars.dialog.is_some() {
            return EventResult::Handled;
        }
        let settings = &self.page.zoom_settings;
        if self.in_page(at).is_none() || !settings.enabled || !settings.mouse_wheel {
            return EventResult::Ignored;
        }
        if scale <= 0.0 || (scale - 1.0).abs() < 1e-6 {
            return EventResult::Handled;
        }
        // The desktop shell sends exp(0.1 × lines) per wheel step: whole
        // steps move between zoom levels, anything else zooms smoothly.
        let lines = scale.ln() / 0.1;
        let delta = if lines.abs() > 0.99 && (lines - lines.round()).abs() < 0.01 {
            ScrollDelta::Lines {
                x: 0.0,
                y: lines.round() as f32,
            }
        } else {
            ScrollDelta::Pixels {
                x: 0.0,
                y: (scale.ln() / 0.0025) as f32,
            }
        };
        self.dispatch(ctx, Inner::Wheel(delta));
        EventResult::Handled
    }

    // ----------------------------------------------------------------
    // Keyboard
    // ----------------------------------------------------------------

    fn key(
        &mut self,
        ctx: &mut EventCtx,
        event: &InputEvent,
        key_event: &frust::authoring::KeyEvent,
    ) -> EventResult {
        let named = |name| key_event.key == frust::authoring::Key::Named(name);
        if self.bars.dialog.is_some() {
            if named(frust::authoring::NamedKey::Escape) {
                self.dispatch(ctx, Inner::LinkCancel);
                return EventResult::Handled;
            }
            if named(frust::authoring::NamedKey::Tab) {
                // Switch between the two fields.
                let on_text = self
                    .text_pod
                    .as_ref()
                    .is_some_and(ChildPod::holds_live_focus);
                self.focus_field(ctx, !on_text && self.text_pod.is_some());
                return EventResult::Handled;
            }
        }
        if let Some(pod) = self.focused_field() {
            let result = route_event_single(pod, ctx, event);
            if result == EventResult::Ignored && named(frust::authoring::NamedKey::Escape) {
                self.dispatch(ctx, Inner::LinkCancel);
                return EventResult::Handled;
            }
            return result;
        }
        if self.page.link_open {
            if key_event.key == frust::authoring::Key::Named(frust::authoring::NamedKey::Escape) {
                self.dispatch(ctx, Inner::LinkCancel);
                return EventResult::Handled;
            }
            return EventResult::Ignored;
        }
        let Some((key, modifiers, text)) = keyboard::from_frust(key_event) else {
            return EventResult::Ignored;
        };
        self.modifiers = modifiers;

        if let Some(command) = self.page.zoom_settings.command_for(&key, &key, modifiers) {
            self.dispatch(ctx, Inner::Command(command));
            return EventResult::Handled;
        }

        if !self.page.focused {
            return EventResult::Ignored;
        }

        if modifiers.command()
            && !modifiers.alt()
            && let Key::Character(c) = key.as_ref()
        {
            let c = c.to_lowercase();
            // Shortcuts of disabled features go to the application.
            let allowed = builtin_command(&c, modifiers.shift())
                .filter(|command| self.page.features.allows(command));
            if allowed.is_none() {
                return EventResult::Ignored;
            }
        }

        let tab = self.page.tab_width > 0 && !modifiers.command();
        let editing_key = match key.as_ref() {
            Key::Named(Named::Tab) => tab,
            Key::Named(
                Named::ArrowLeft
                | Named::ArrowRight
                | Named::ArrowUp
                | Named::ArrowDown
                | Named::Home
                | Named::End
                | Named::PageUp
                | Named::PageDown
                | Named::Backspace
                | Named::Delete
                | Named::Enter
                | Named::Escape,
            ) => true,
            Key::Named(Named::Copy | Named::Cut | Named::Paste) => true,
            Key::Character(_) => true,
            _ => false,
        };
        let types_text = text
            .as_ref()
            .is_some_and(|text| !text.is_empty() && text.chars().all(|c| !c.is_control()));
        if !editing_key && !types_text {
            return EventResult::Ignored;
        }

        // Dedicated clipboard keys act like the shortcuts.
        let command = match key.as_ref() {
            Key::Named(Named::Copy) => Some(Command::Copy),
            Key::Named(Named::Cut) => Some(Command::Cut),
            Key::Named(Named::Paste) => Some(Command::Paste),
            _ => None,
        };
        match command {
            Some(command) => self.dispatch(ctx, Inner::Command(command)),
            None => self.dispatch(ctx, Inner::Key(key, modifiers, text)),
        }
        EventResult::Handled
    }

    /// Text from an input method: composed (shown at the caret until it is
    /// finished) or committed. Mobile keyboards compose whole words.
    fn ime(&mut self, ctx: &mut EventCtx, event: &InputEvent, ime: &ImeEvent) -> EventResult {
        if let Some(pod) = self.focused_field() {
            return route_event_single(pod, ctx, event);
        }
        if !self.page.focused || self.page.link_open {
            return EventResult::Ignored;
        }
        match ime {
            ImeEvent::Compose { text, .. } => self.dispatch(ctx, Inner::Compose(text.clone())),
            ImeEvent::Commit(text) => self.dispatch(ctx, Inner::Commit(text.clone())),
            // The session ended: what was composed stays.
            ImeEvent::Disabled => self.dispatch(ctx, Inner::EndCompose),
            _ => return EventResult::Ignored,
        }
        EventResult::Handled
    }

    fn edit_command(
        &mut self,
        ctx: &mut EventCtx,
        event: &InputEvent,
        command: &EditCommand,
    ) -> EventResult {
        if let Some(pod) = self.focused_field() {
            return route_event_single(pod, ctx, event);
        }
        if self.page.link_open {
            return EventResult::Ignored;
        }
        match command {
            EditCommand::Copy => self.dispatch(ctx, Inner::Command(Command::Copy)),
            EditCommand::Cut => self.dispatch(ctx, Inner::Command(Command::Cut)),
            EditCommand::SelectAll => self.dispatch(ctx, Inner::Command(Command::SelectAll)),
            EditCommand::Paste(text) => self.dispatch(ctx, Inner::Paste(Some(text.clone()))),
        }
        EventResult::Handled
    }

    // ----------------------------------------------------------------
    // Painting
    // ----------------------------------------------------------------

    /// Whether the blinking caret is shown now, and when that changes.
    fn caret_phase(&self) -> (bool, Duration) {
        let period = super::BLINK.as_millis().max(1);
        let elapsed = self.page.last_activity.elapsed().as_millis();
        let on = (elapsed / period).is_multiple_of(2);
        let left = period - elapsed % period;
        (on, Duration::from_millis(left as u64))
    }

    fn paint_page(&self, ctx: &mut PaintCtx, scene: &mut dyn PaintScene) {
        let page = &self.page;
        let rect = self.page_rect;
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let (sx, sy) = page.scroll;
        let origin = ctx.origin();
        let at = |x: f32, y: f32| {
            frust::kurbo::Affine::translate((origin.x + x as f64, origin.y + y as f64))
        };

        scene.push_transform(at(rect.x, rect.y));
        scene.push_clip(frust::kurbo::Point::ZERO, rect.size().to_kurbo());

        let mut desk = draw::frame(rect.size());
        render::draw_desk(&mut desk, &page.layout, &page.palette, rect.size());
        draw::replay(desk.ops(), scene, None);

        // Document content, culled to what is visible.
        scene.push_transform(frust::kurbo::Affine::translate((-sx as f64, -sy as f64)));
        let visible = Rectangle::new(Point::new(sx, sy), rect.size());
        draw::replay(&page.scene, scene, Some(visible));

        // Selection and caret.
        let mut overlay = draw::frame(rect.size());
        render::draw_selection(&mut overlay, &page.layout, page.sel, &page.palette);
        if page.focused && !page.link_open && self.caret_phase().0 {
            render::draw_caret(&mut overlay, &page.layout, page.sel, &page.palette);
        }
        draw::replay(overlay.ops(), scene, Some(visible));
        scene.pop_transform();

        // Scrollbars.
        let (vertical, horizontal) = scrollbars(page, rect.size());
        let mut bars = draw::frame(rect.size());
        for (bar, dragging) in [
            (vertical, matches!(self.drag, Drag::Vertical(_))),
            (horizontal, matches!(self.drag, Drag::Horizontal(_))),
        ] {
            let Some(bar) = bar else {
                continue;
            };
            let active = dragging
                || self
                    .hover_point
                    .is_some_and(|point| bar.track.contains(point));
            let alpha = if active { 0.55 } else { 0.30 };
            let inset = 2.0;
            bars.fill_rectangle(
                Point::new(bar.thumb.x + inset, bar.thumb.y + inset),
                Size::new(
                    bar.thumb.width - 2.0 * inset,
                    bar.thumb.height - 2.0 * inset,
                ),
                crate::geometry::Color {
                    a: alpha,
                    ..page.palette.muted
                },
            );
        }
        draw::replay(bars.ops(), scene, None);

        scene.pop_clip();
        scene.pop_transform();
    }

    fn paint_bars(&self, ctx: &mut PaintCtx, scene: &mut dyn PaintScene) {
        let origin = ctx.origin();
        scene.push_transform(frust::kurbo::Affine::translate((origin.x, origin.y)));
        let mut frame = draw::frame(self.size);
        for (id, bar, (rect, slots)) in [
            (BarId::Toolbar, &self.bars.toolbar, &self.toolbar),
            (BarId::Link, &self.bars.link, &self.link_bar),
            (BarId::Status, &self.bars.status, &self.status),
        ] {
            if let Some(bar) = bar {
                chrome::draw(
                    &mut frame,
                    bar,
                    *rect,
                    slots,
                    &self.page.palette,
                    self.slot_status(id),
                );
            }
        }
        // A toolbar scrolled sideways fades out where more buttons wait.
        if self.toolbar_max > 0.0 {
            let rect = self.toolbar.0;
            let background = self.page.palette.toolbar_background;
            let steps = 8;
            let step = 3.0;
            for i in 0..steps {
                let alpha = 1.0 - i as f32 / steps as f32;
                let color = background.scale_alpha(alpha);
                let inset = Size::new(step, (rect.height - 2.0).max(0.0));
                let offset = i as f32 * step;
                if self.toolbar_scroll > 0.5 {
                    frame.fill_rectangle(
                        Point::new(rect.x + 1.0 + offset, rect.y + 1.0),
                        inset,
                        color,
                    );
                }
                if self.toolbar_scroll < self.toolbar_max - 0.5 {
                    let x = rect.x + rect.width - 1.0 - offset - step;
                    frame.fill_rectangle(Point::new(x, rect.y + 1.0), inset, color);
                }
            }
        }
        draw::replay(frame.ops(), scene, None);
        scene.pop_transform();
    }

    /// The open drop-down list and the context menu.
    fn paint_menu(&self, ctx: &mut PaintCtx, scene: &mut dyn PaintScene) {
        let origin = ctx.origin();
        scene.push_transform(frust::kurbo::Affine::translate((origin.x, origin.y)));
        let mut frame = draw::frame(self.size);
        if let Some((dropdown, menu)) = self.open_menu() {
            let hovered = self.menu.and_then(|menu| menu.hovered);
            chrome::draw_menu(&mut frame, &dropdown, &menu, hovered, &self.page.palette);
        }
        if let Some(context) = &self.bars.context {
            context.draw(
                &mut frame,
                self.size,
                self.context_hover,
                &self.page.palette,
            );
        }
        draw::replay(frame.ops(), scene, None);
        scene.pop_transform();
    }

    /// The link dialog's backdrop and box (its fields are painted after).
    fn paint_dialog(&self, ctx: &mut PaintCtx, scene: &mut dyn PaintScene) {
        let (Some(view), Some(layout)) = (&self.bars.dialog, &self.dialog) else {
            return;
        };
        let origin = ctx.origin();
        scene.push_transform(frust::kurbo::Affine::translate((origin.x, origin.y)));
        let mut frame = draw::frame(self.size);
        dialog::draw(
            &mut frame,
            view,
            layout,
            self.size,
            &self.page.palette,
            self.slot_status(BarId::Dialog),
        );
        draw::replay(frame.ops(), scene, None);
        scene.pop_transform();
    }

    /// Pointer state of the slots of `bar`.
    fn slot_status(&self, bar: BarId) -> impl Fn(usize) -> Status + '_ {
        let pressed = match self.drag {
            Drag::Slot(slot) => Some(slot),
            Drag::Toolbar {
                slot,
                panning: false,
                ..
            } => slot,
            _ => None,
        };
        move |index: usize| {
            if pressed == Some((bar, index)) {
                if self.hovered == Some((bar, index)) {
                    Status::Pressed
                } else {
                    Status::Idle
                }
            } else if (self.hovered == Some((bar, index)) && self.drag == Drag::None)
                || self.menu.is_some_and(|menu| menu.slot == (bar, index))
            {
                Status::Hovered
            } else {
                Status::Idle
            }
        }
    }
}

impl Widget for EditorWidget {
    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints) -> frust::kurbo::Size {
        let max = bc.max();
        let wanted = frust::kurbo::Size::new(
            self.width.resolve(max.width, FALLBACK.width) as f64,
            self.height.resolve(max.height, FALLBACK.height) as f64,
        );
        let constrained = bc.constrain(wanted);
        let size = Size::new(constrained.width as f32, constrained.height as f32);
        self.size = size;
        let width = size.width;
        let mut y = 0.0;

        self.toolbar = match &self.bars.toolbar {
            Some(bar) => {
                // Narrow editors (phones): one row that scrolls sideways
                // rather than many rows taking the room of the page.
                let scrolls = bar.wrap && chrome::line_count(bar, width, 0.0) > TOOLBAR_LINES;
                let (height, slots) = if scrolls {
                    let row = Bar {
                        wrap: false,
                        ..bar.clone()
                    };
                    let content = chrome::natural_width(&row, 0.0);
                    self.toolbar_max = (content - width).max(0.0);
                    self.toolbar_scroll = self.toolbar_scroll.min(self.toolbar_max);
                    chrome::layout(&row, Point::new(-self.toolbar_scroll, y), content, 0.0)
                } else {
                    self.toolbar_max = 0.0;
                    self.toolbar_scroll = 0.0;
                    chrome::layout(bar, Point::new(0.0, y), width, 0.0)
                };
                let rect = Rectangle::new(Point::new(0.0, y), Size::new(width, height));
                y += height;
                (rect, slots)
            }
            None => {
                self.toolbar_max = 0.0;
                self.toolbar_scroll = 0.0;
                (Rectangle::default(), Vec::new())
            }
        };

        self.link_bar = match (&self.bars.link, self.link_pod.as_mut()) {
            (Some(bar), Some(pod)) => {
                let input_width = toolbar::LINK_INPUT_WIDTH.min((width - 24.0).max(60.0));
                let input = pod.layout_child(
                    ctx,
                    &BoxConstraints::new(
                        frust::kurbo::Size::new(input_width as f64, 0.0),
                        frust::kurbo::Size::new(input_width as f64, f64::INFINITY),
                    ),
                );
                let mut bar = bar.clone();
                for item in &mut bar.items {
                    if let Item::Input { width } = item {
                        *width = input.width as f32;
                    }
                }
                let (height, slots) =
                    chrome::layout(&bar, Point::new(0.0, y), width, input.height as f32);
                if let Some(slot) = slots.iter().find(|s| matches!(s.item, Item::Input { .. })) {
                    pod.set_origin(slot.rect.position().to_kurbo());
                }
                let rect = Rectangle::new(Point::new(0.0, y), Size::new(width, height));
                y += height;
                (rect, slots)
            }
            _ => (Rectangle::default(), Vec::new()),
        };

        self.status = match &self.bars.status {
            Some(bar) => {
                // Narrow editors leave out the less important labels.
                let bar = &chrome::fit(bar, width, 0.0);
                let (height, _) = chrome::layout(bar, Point::ORIGIN, width, 0.0);
                let top = (size.height - height).max(y);
                let (_, slots) = chrome::layout(bar, Point::new(0.0, top), width, 0.0);
                (
                    Rectangle::new(Point::new(0.0, top), Size::new(width, height)),
                    slots,
                )
            }
            None => (
                Rectangle::new(Point::new(0.0, size.height), Size::ZERO),
                Vec::new(),
            ),
        };

        self.dialog = self.bars.dialog.as_ref().map(|view| {
            let width = dialog::field_width(size.width) as f64;
            let constraints = BoxConstraints::new(
                frust::kurbo::Size::new(width, 0.0),
                frust::kurbo::Size::new(width, f64::INFINITY),
            );
            let mut height: f32 = 0.0;
            for pod in [self.link_pod.as_mut(), self.text_pod.as_mut()]
                .into_iter()
                .flatten()
            {
                height = height.max(pod.layout_child(ctx, &constraints).height as f32);
            }
            let layout = dialog::layout(view, size, height);
            if let Some(pod) = self.link_pod.as_mut() {
                pod.set_origin(layout.url_field.to_kurbo());
            }
            if let (Some(pod), Some(at)) = (self.text_pod.as_mut(), layout.text_field) {
                pod.set_origin(at.to_kurbo());
            }
            layout
        });

        let bottom = self.status.0.y.max(y);
        self.page_rect =
            Rectangle::new(Point::new(0.0, y), Size::new(width, (bottom - y).max(0.0)));
        {
            let mut link = self.link.lock().unwrap_or_else(|p| p.into_inner());
            link.reported_size = Some(self.page_rect.size());
        }
        constrained
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut dyn PaintScene) {
        if !ctx.is_hovered() && self.drag == Drag::None && self.hovered.is_some() {
            self.hovered = None;
            self.hover_point = None;
        }

        let origin = ctx.origin();
        scene.push_clip(origin, self.size.to_kurbo());
        self.paint_page(ctx, scene);
        self.paint_bars(ctx, scene);
        if self.bars.dialog.is_some() {
            self.paint_menu(ctx, scene);
            self.paint_dialog(ctx, scene);
            for pod in [self.text_pod.as_mut(), self.link_pod.as_mut()]
                .into_iter()
                .flatten()
            {
                pod.paint_child(ctx, scene);
            }
        } else {
            if let Some(pod) = self.link_pod.as_mut() {
                pod.paint_child(ctx, scene);
            }
            self.paint_menu(ctx, scene);
        }
        scene.pop_clip();

        // The layout was made for another size: build again next frame.
        let page_size = self.page_rect.size();
        if (page_size.width - self.page.viewport.width).abs() > 0.5
            || (page_size.height - self.page.viewport.height).abs() > 0.5
        {
            ctx.request_frame();
        }

        // Blink while focused.
        if self.page.focused && !self.page.link_open {
            let (_, next) = self.caret_phase();
            ctx.request_frame_paced_at(next.max(Duration::from_millis(16)));
        }

        // Typing in the page (not in a link field): the on-screen keyboard
        // and the input method, next to the caret.
        if self.page.focused
            && !self.page.link_open
            && self.bars.dialog.is_none()
            && !self.input_focused()
        {
            let (sx, sy) = self.page.scroll;
            let caret = self
                .page
                .layout
                .caret(self.page.sel.head)
                .map(|(x, y, height)| {
                    let left = origin.x + f64::from(self.page_rect.x + x - sx);
                    let top = origin.y + f64::from(self.page_rect.y + y - sy);
                    frust::kurbo::Rect::new(left, top, left + 1.0, top + f64::from(height))
                });
            ctx.publish_ime_state(ime_state(caret));
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &InputEvent) -> EventResult {
        match event {
            InputEvent::Housekeeping => {
                for pod in [self.link_pod.as_mut(), self.text_pod.as_mut()]
                    .into_iter()
                    .flatten()
                {
                    route_event_single(pod, ctx, event);
                }
                self.housekeeping(ctx);
                EventResult::Ignored
            }
            InputEvent::Pointer(p) => self.pointer(ctx, event, p),
            InputEvent::Scroll { position, delta } => self.scroll(ctx, point(*position), *delta),
            InputEvent::Scale(scale) => self.pinch(ctx, point(scale.focal), scale.scale_delta),
            InputEvent::Key(key) => self.key(ctx, event, key),
            InputEvent::Ime(ime) => self.ime(ctx, event, ime),
            InputEvent::EditCommand(command) => self.edit_command(ctx, event, command),
            InputEvent::FileDrop(drop) => {
                if !self.page.accepts_drops {
                    return EventResult::Ignored;
                }
                match drop.phase {
                    FileDropPhase::Drop => {
                        for path in &drop.paths {
                            self.dispatch(ctx, Inner::FileDropped(path.clone()));
                        }
                        EventResult::Handled
                    }
                    FileDropPhase::Hover => EventResult::Handled,
                    FileDropPhase::Cancel | FileDropPhase::Ended => EventResult::Ignored,
                }
            }
            _ => {
                for pod in [self.link_pod.as_mut(), self.text_pod.as_mut()]
                    .into_iter()
                    .flatten()
                {
                    let result = route_event_single(pod, ctx, event);
                    if result == EventResult::Handled {
                        return result;
                    }
                }
                EventResult::Ignored
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        for pod in [&self.text_pod, &self.link_pod].into_iter().flatten() {
            pod.semantics_child(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn FnMut(&ChildPod)) {
        for pod in [&self.text_pod, &self.link_pod].into_iter().flatten() {
            visitor(pod);
        }
    }

    fn is_focusable(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
