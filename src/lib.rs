//! # frust-markdown-editor
//!
//! A Word-style WYSIWYG CommonMark editor widget for
//! [Frust](https://frust.dev), with the GFM extensions strikethrough, task
//! lists and tables. Desktop and web.
//!
//! Keep an [`EditorState`] in your component state, build the widget with
//! [`markdown_editor`] and hand its [`Action`]s to [`EditorState::update`]:
//!
//! ```no_run
//! use frust::{Component, View};
//! use frust_markdown_editor::{EditorState, Event, markdown_editor};
//!
//! #[derive(Default)]
//! struct App;
//!
//! struct State {
//!     editor: EditorState,
//! }
//!
//! impl Component for App {
//!     type State = State;
//!
//!     fn init(&self) -> State {
//!         State {
//!             editor: EditorState::with_markdown("# Hello\n\n- [ ] first task\n"),
//!         }
//!     }
//!
//!     fn build(&self, state: &mut State) -> impl View<State> {
//!         markdown_editor(&state.editor).view(|state: &mut State, action| {
//!             let output = state.editor.update(action);
//!             if output.events.contains(&Event::Changed) {
//!                 println!("{}", state.editor.markdown());
//!             }
//!         })
//!     }
//! }
//!
//! frust::app!(App);
//! # fn main() {}
//! ```
//!
//! The widget draws the page, toolbar and status bar itself and handles the
//! caret blinking, clipboard and dropped files; there is no subscription or
//! task to wire up. Inside a Frust `column()` or `row()`, add it with
//! `.flex(1, …)` so it gets the remaining room.
//!
//! ## Features
//!
//! | Feature | Default | Adds |
//! |---|---|---|
//! | `syntect` | off | [`SyntectHighlighter`], a ready-made code highlighter |
//! | `images` | off | Showing images, and pasting images from the clipboard (desktop) |
//!
//! ## Tables
//!
//! GFM tables are edited in place: [`Command::InsertTable`], row and column
//! commands, [`Command::SetColumnAlign`] with [`ColumnAlign`]. `Tab` moves to
//! the next cell, `Enter` to the cell below.
//!
//! ## Switching features off
//!
//! Tables, task lists and strikethrough can be disabled at runtime with
//! [`Features`] ([`EditorState::features`]).
//!
//! ## Theme
//!
//! Colours and font sizes come from an [`EditorStyle`]: a preset, one
//! derived from the application's Frust theme, or your own [`Palette`] and
//! [`Typography`] ([`EditorState::style`]).
//!
//! ```
//! use frust_markdown_editor::{EditorState, EditorStyle, FromTheme};
//!
//! let editor = EditorState::new().style(EditorStyle::dark());
//!
//! // Match the rest of the application.
//! let theme = frust::Theme::neutral().with_brightness(frust::Brightness::Dark);
//! let editor = EditorState::new().style(EditorStyle::from_theme(&theme));
//! ```
//!
//! [`EditorState::set_style`] changes it at runtime, e.g. when the
//! application theme changes.
//!
//! ## Languages
//!
//! The toolbar, link bar and status bar speak English, German, French,
//! Spanish and Italian ([`Language`], [`EditorState::language`]). The
//! document itself is never translated.
//!
//! ```
//! use frust_markdown_editor::{EditorState, Language};
//!
//! // E.g. from the operating system locale; unknown tags fall back to English.
//! let language = Language::from_tag("de-CH").unwrap_or_default();
//! let editor = EditorState::new().language(language);
//! ```
//!
//! [`EditorState::set_language`] switches it at runtime.
//!
//! ## Extensions
//!
//! A [`BlockRenderer`] (from your own crate) draws fenced code blocks of
//! chosen languages, e.g. formulas or diagrams
//! ([`EditorState::block_renderer`]). A [`CodeHighlighter`] colours code.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub(crate) mod draw;
pub(crate) mod fonts;
pub mod geometry;
pub mod i18n;
pub mod images;
pub mod keyboard;
mod platform;
pub mod style;
pub mod widget;

pub use markdown_editor_core::{extension, features, highlight, page, zoom};

pub(crate) use markdown_editor_core::{doc, layout, render};

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub use markdown_editor_core::syntect;

pub use extension::{BlockRenderer, RenderRequest};
pub use features::Features;
pub use fonts::{FONTS, MONO_FAMILY, SANS_FAMILY};
pub use geometry::{Color, Point, Rectangle, Size, Vector};
pub use highlight::{CodeHighlighter, HighlightRequest, HighlightSpan};
pub use i18n::Language;
pub use images::{
    ImageId, ImageLoader, ImageMode, ImageRejection, ImageRejectionMessage, ImageSettings,
    ImageSource, PastedImage,
};
pub use keyboard::{Key, Modifiers, Named};
pub use markdown_editor_core::{BlockKind, ColumnAlign, normalize};
pub use page::{IsoSeries, Margins, Measure, Orientation, PageSize, Paper, ParseError, Unit};
pub use style::{EditorStyle, FromTheme, Palette, Typography};
pub use widget::{
    Action, Command, DEFAULT_TAB_WIDTH, EditorCore, EditorState, EditorView, Event, Formatting,
    Length, MarkdownEditor, Mode, Output, markdown_editor,
};
pub use zoom::{KeyBinding, ZoomSettings, ZoomShortcuts};

#[cfg(feature = "syntect")]
#[cfg_attr(docsrs, doc(cfg(feature = "syntect")))]
pub use markdown_editor_core::SyntectHighlighter;
