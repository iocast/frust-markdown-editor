//! Full demo of the `markdown_editor` widget (desktop).
//!
//! Bar 1: file handling, theme, interface language, Tab width, syntax
//!        highlighting, event log.
//! Bar 2: full width (no paper), paper size, orientation, margins, zoom.
//! Images are saved in `assets/` next to the saved document.
//! Shortcuts: Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S.
//!
//! cargo run --example editor --features syntect,images

#[path = "common/mod.rs"]
mod common;

use common::{Inbox, action, action_if, bar, drain, inbox, perform, pick_list, toggle};
use frust::{
    AnyView, Brightness, Component, DesktopConfig, Theme, View, any, column, divider, shortcuts,
    text,
};
use frust_markdown_editor::{
    CodeHighlighter, Command, EditorState, EditorStyle, Event, ImageId, ImageSettings, KeyBinding,
    Language, Margins, Measure, Orientation, Output, PageSize, Paper, PastedImage,
    SyntectHighlighter, ZoomSettings, ZoomShortcuts, markdown_editor,
};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SAMPLE: &str = r#"# frust-markdown-editor

A Word-style **WYSIWYG** editor for *CommonMark*, written in Rust with
[Frust](https://frust.dev "Frust website").

## Task list

- [x] Write the request
- [ ] Post it
  - [ ] wait for the mods
  - [ ] answer questions
- [ ] Share the first release

Click a box, or put the caret in an item and press **Ctrl+Enter**.

## Images

Save this document, then paste a screenshot (Ctrl+V), drop an image file onto
the window, or use the **Image** button. Images are stored in `assets/`.

## Structure

1. Nested lists
   - press **Tab** at the start of an item to nest it
   - **Shift+Tab** moves it out again
2. Items with several paragraphs

   This paragraph belongs to the second item.

> Block quotes can contain lists:
>
> - one
> - two

## Tables

| Key       | In a table                    |  Mode |
| :-------- | :---------------------------: | ----: |
| **Tab**   | next cell, a new row at the end | both |
| **Enter** | cell below, leave on an empty row | WYSIWYG |
| `\|`      | a pipe inside a cell          |   any |

Type `| a | b |` and press **Enter** to start a table.

## Code

```rust
use frust::{Component, View};
use frust_markdown_editor::{markdown_editor, EditorState};

impl Component for App {
    type State = State;

    fn build(&self, state: &mut State) -> impl View<State> {
        markdown_editor(&state.editor).view(|state: &mut State, action| {
            state.editor.update(action);
        })
    }
}
```
"#;

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name(format!(
            "frust-markdown-editor {}",
            env!("CARGO_PKG_VERSION")
        ))
        .with_initial_size((1360.0, 940.0));
    if let Err(error) = frust::run_desktop_config(Demo, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------------
// Choices
// ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum ThemeChoice {
    WordLight,
    WordDark,
    FrustLight,
    FrustDark,
}

impl ThemeChoice {
    const ALL: [ThemeChoice; 4] = [
        ThemeChoice::WordLight,
        ThemeChoice::WordDark,
        ThemeChoice::FrustLight,
        ThemeChoice::FrustDark,
    ];

    fn app_theme(self) -> Theme {
        let brightness = match self {
            ThemeChoice::WordLight | ThemeChoice::FrustLight => Brightness::Light,
            ThemeChoice::WordDark | ThemeChoice::FrustDark => Brightness::Dark,
        };
        Theme::neutral().with_brightness(brightness)
    }

    fn editor_style(self) -> EditorStyle {
        match self {
            ThemeChoice::WordLight => EditorStyle::light(),
            ThemeChoice::WordDark => EditorStyle::dark(),
            ThemeChoice::FrustLight | ThemeChoice::FrustDark => {
                EditorStyle::from_theme(&self.app_theme())
            }
        }
    }
}

impl fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ThemeChoice::WordLight => "Word light",
            ThemeChoice::WordDark => "Word dark",
            ThemeChoice::FrustLight => "Frust: light",
            ThemeChoice::FrustDark => "Frust: dark",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabWidth(usize);

const TAB_WIDTHS: [TabWidth; 4] = [TabWidth(0), TabWidth(2), TabWidth(4), TabWidth(8)];

impl fmt::Display for TabWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            0 => f.write_str("Tab off"),
            n => write!(f, "Tab = {n} spaces"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaperChoice {
    A3,
    A4,
    A5,
    B5,
    Letter,
    Legal,
    Custom,
}

impl PaperChoice {
    const ALL: [PaperChoice; 7] = [
        PaperChoice::A3,
        PaperChoice::A4,
        PaperChoice::A5,
        PaperChoice::B5,
        PaperChoice::Letter,
        PaperChoice::Legal,
        PaperChoice::Custom,
    ];

    fn size(self) -> PageSize {
        match self {
            PaperChoice::A3 => PageSize::A3,
            PaperChoice::A4 => PageSize::A4,
            PaperChoice::A5 => PageSize::A5,
            PaperChoice::B5 => PageSize::B5,
            PaperChoice::Letter => PageSize::Letter,
            PaperChoice::Legal => PageSize::Legal,
            PaperChoice::Custom => PageSize::custom(Measure::cm(15.0), Measure::cm(21.0)),
        }
    }
}

impl fmt::Display for PaperChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PaperChoice::A3 => "A3 (297 x 420 mm)",
            PaperChoice::A4 => "A4 (210 x 297 mm)",
            PaperChoice::A5 => "A5 (148 x 210 mm)",
            PaperChoice::B5 => "B5 (176 x 250 mm)",
            PaperChoice::Letter => "Letter (8.5 x 11 in)",
            PaperChoice::Legal => "Legal (8.5 x 14 in)",
            PaperChoice::Custom => "Custom (15 x 21 cm)",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MarginChoice {
    Normal,
    Narrow,
    Moderate,
    Wide,
    Minimal,
}

impl MarginChoice {
    const ALL: [MarginChoice; 5] = [
        MarginChoice::Normal,
        MarginChoice::Narrow,
        MarginChoice::Moderate,
        MarginChoice::Wide,
        MarginChoice::Minimal,
    ];

    fn margins(self) -> Margins {
        match self {
            MarginChoice::Normal => Margins::all(Measure::cm(2.5)),
            MarginChoice::Narrow => Margins::all(Measure::cm(1.27)),
            MarginChoice::Moderate => Margins::symmetric(Measure::cm(2.54), Measure::cm(1.91)),
            MarginChoice::Wide => Margins::symmetric(Measure::cm(2.54), Measure::cm(5.08)),
            MarginChoice::Minimal => Margins::all(Measure::mm(5.0)),
        }
    }
}

impl fmt::Display for MarginChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MarginChoice::Normal => "Normal margins",
            MarginChoice::Narrow => "Narrow margins",
            MarginChoice::Moderate => "Moderate margins",
            MarginChoice::Wide => "Wide margins",
            MarginChoice::Minimal => "Minimal margins",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZoomLevel(u32);

const ZOOM_LEVELS: [ZoomLevel; 11] = [
    ZoomLevel(25),
    ZoomLevel(50),
    ZoomLevel(75),
    ZoomLevel(90),
    ZoomLevel(100),
    ZoomLevel(110),
    ZoomLevel(125),
    ZoomLevel(150),
    ZoomLevel(200),
    ZoomLevel(300),
    ZoomLevel(400),
];

impl fmt::Display for ZoomLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} %", self.0)
    }
}

fn zoom_settings(enabled: bool) -> ZoomSettings {
    ZoomSettings {
        enabled,
        shortcuts: ZoomShortcuts {
            zoom_in: vec![KeyBinding::command("+"), KeyBinding::command("=")],
            zoom_out: vec![KeyBinding::command("-")],
            reset: vec![KeyBinding::command("0")],
        },
        ..ZoomSettings::default()
    }
}

// ------------------------------------------------------------------------
// Application
// ------------------------------------------------------------------------

struct Demo;

struct State {
    editor: EditorState,
    highlighter: Arc<dyn CodeHighlighter>,
    highlighting: bool,
    tab_width: TabWidth,
    path: Option<PathBuf>,
    theme: ThemeChoice,
    paper: PaperChoice,
    orientation: Orientation,
    margins: MarginChoice,
    full_width: bool,
    zoom_enabled: bool,
    log: Vec<String>,
    /// The open drop-down list, if any.
    open: Option<&'static str>,
    /// Results of file dialogs and file operations.
    done: Inbox<Done>,
}

/// Background work that finished.
enum Done {
    Opened(Option<PathBuf>),
    Loaded(Result<(PathBuf, String), String>),
    Saved(Result<PathBuf, String>),
    ImagePicked(Option<PathBuf>),
    ImageStored(ImageId, Result<String, String>),
}

/// Writes an image into `<folder>/assets/` and returns the relative URL.
async fn store_image(folder: PathBuf, image: PastedImage) -> Result<String, String> {
    let assets = folder.join("assets");
    std::fs::create_dir_all(&assets).map_err(|error| error.to_string())?;
    let name = image.file_name();
    std::fs::write(assets.join(&name), &*image.bytes).map_err(|error| error.to_string())?;
    Ok(format!("assets/{name}"))
}

async fn pick(name: &'static str, extensions: &'static [&'static str]) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter(name, extensions)
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
}

impl Component for Demo {
    type State = State;

    fn init(&self) -> State {
        let highlighter: Arc<dyn CodeHighlighter> = Arc::new(SyntectHighlighter::new());
        let mut state = State {
            editor: EditorState::with_markdown(SAMPLE)
                .zoom_settings(zoom_settings(true))
                .images(ImageSettings::delegate())
                .tab_width(4),
            highlighter,
            highlighting: true,
            tab_width: TabWidth(4),
            path: None,
            theme: ThemeChoice::WordLight,
            paper: PaperChoice::A4,
            orientation: Orientation::Portrait,
            margins: MarginChoice::Normal,
            full_width: false,
            zoom_enabled: true,
            log: Vec::new(),
            open: None,
            done: inbox(),
        };
        state
            .editor
            .set_highlighter(Some(state.highlighter.clone()));
        state.set_theme(ThemeChoice::WordLight);
        state.apply_paper();
        state
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        for done in drain(state.done) {
            state.finished(done);
        }
        let set_open = |state: &mut State, open| state.open = open;

        let file_bar = bar(vec![
            any(action("New", |state: &mut State| state.new_document())),
            any(action("Open", |state: &mut State| state.open())),
            any(action("Save", |state: &mut State| state.save(false))),
            any(action("Save as", |state: &mut State| state.save(true))),
            any(pick_list(
                "theme",
                state.open,
                ThemeChoice::ALL.to_vec(),
                state.theme,
                set_open,
                |state: &mut State, choice| {
                    state.set_theme(choice);
                    state.log(format!("Theme: {choice}"));
                },
            )),
            any(pick_list(
                "language",
                state.open,
                Language::ALL.to_vec(),
                state.editor.current_language(),
                set_open,
                |state: &mut State, language| {
                    state.editor.set_language(language);
                    state.log(format!("Language: {language}"));
                },
            )),
            any(pick_list(
                "tab",
                state.open,
                TAB_WIDTHS.to_vec(),
                state.tab_width,
                set_open,
                |state: &mut State, width| {
                    state.tab_width = width;
                    state.editor.set_tab_width(width.0);
                    state.log(width.to_string());
                },
            )),
            any(toggle(
                "Syntax highlighting",
                state.highlighting,
                |state: &mut State| {
                    state.highlighting = !state.highlighting;
                    let highlighter = state.highlighting.then(|| state.highlighter.clone());
                    state.editor.set_highlighter(highlighter);
                },
            )),
            any(text(state.title()).size(12.0)),
            any(text(state.log.join("   \u{00B7}   ")).size(12.0)),
        ]);

        let zoom_on = state.zoom_enabled;
        let zoom = move |label: &str, command: Command| {
            any(action_if(label, zoom_on, move |state: &mut State| {
                let output = state.editor.execute(command.clone());
                state.handle(output);
            }))
        };
        let percent = ZoomLevel((state.editor.current_zoom() * 100.0).round() as u32);
        let paged = !state.full_width;
        let orientation = match state.orientation {
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        };

        let mut view_items: Vec<AnyView<State>> = vec![any(toggle(
            "Full width (no paper)",
            state.full_width,
            |state: &mut State| {
                state.full_width = !state.full_width;
                state.apply_paper();
            },
        ))];
        if paged {
            view_items.push(any(pick_list(
                "paper",
                state.open,
                PaperChoice::ALL.to_vec(),
                state.paper,
                set_open,
                |state: &mut State, choice| {
                    state.paper = choice;
                    state.full_width = false;
                    state.apply_paper();
                },
            )));
            view_items.push(any(action(orientation, |state: &mut State| {
                state.orientation = match state.orientation {
                    Orientation::Portrait => Orientation::Landscape,
                    Orientation::Landscape => Orientation::Portrait,
                };
                state.full_width = false;
                state.apply_paper();
            })));
            view_items.push(any(pick_list(
                "margins",
                state.open,
                MarginChoice::ALL.to_vec(),
                state.margins,
                set_open,
                |state: &mut State, choice| {
                    state.margins = choice;
                    state.apply_paper();
                },
            )));
        }
        view_items.push(any(toggle(
            "Zoom",
            state.zoom_enabled,
            |state: &mut State| {
                state.zoom_enabled = !state.zoom_enabled;
                state
                    .editor
                    .set_zoom_settings(zoom_settings(state.zoom_enabled));
            },
        )));
        view_items.push(zoom("\u{2212}", Command::ZoomOut));
        view_items.push(any(pick_list(
            "zoom",
            state.open,
            ZOOM_LEVELS.to_vec(),
            percent,
            set_open,
            |state: &mut State, level| {
                let output = state
                    .editor
                    .execute(Command::SetZoom(level.0 as f32 / 100.0));
                state.handle(output);
            },
        )));
        view_items.push(zoom("+", Command::ZoomIn));
        view_items.push(zoom("100 %", Command::ResetZoom));
        if paged {
            view_items.push(zoom("Fit width", Command::FitWidth));
            view_items.push(zoom("Fit page", Command::FitPage));
        }
        view_items.push(any(text(state.summary()).size(12.0)));

        let editor = markdown_editor(&state.editor).view(|state: &mut State, action| {
            let output = state.editor.update(action);
            state.handle(output);
        });

        shortcuts(
            column()
                .child(file_bar)
                .child(bar(view_items))
                .child(divider(frust::Color::from_rgb8(0xCC, 0xCE, 0xD4)))
                .flex(1, editor),
        )
        .on("Mod+N", "New", |state: &mut State| state.new_document())
        .on("Mod+O", "Open", |state: &mut State| state.open())
        .on("Mod+S", "Save", |state: &mut State| state.save(false))
        .on("Mod+Shift+S", "Save as", |state: &mut State| {
            state.save(true)
        })
    }
}

impl State {
    fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled.md");
        let dirty = if self.editor.is_dirty() {
            "\u{25CF} "
        } else {
            ""
        };
        format!("{dirty}{name}")
    }

    fn log(&mut self, entry: String) {
        self.log.push(entry);
        if self.log.len() > 3 {
            self.log.remove(0);
        }
    }

    fn folder(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    fn set_theme(&mut self, choice: ThemeChoice) {
        self.theme = choice;
        self.editor.set_style(choice.editor_style());
        frust::set_app_theme(choice.app_theme());
    }

    fn apply_paper(&mut self) {
        let paper = if self.full_width {
            Paper::fill()
        } else {
            Paper::new(self.paper.size())
                .with_orientation(self.orientation)
                .with_margins(self.margins.margins())
        };
        self.editor.set_paper(paper);
    }

    /// Reacts to editor events, also for commands run by the demo itself.
    fn handle(&mut self, output: Output) {
        for event in output.events {
            match event {
                Event::SelectionChanged | Event::Changed => {}
                Event::ZoomChanged(zoom) => self.log(format!("Zoom {:.0} %", zoom * 100.0)),
                Event::LinkClicked(url) => self.log(format!("Link clicked: {url}")),
                Event::ImagePasted(image) => match self.folder() {
                    Some(folder) => {
                        let id = image.id;
                        perform(self.done, async move {
                            Done::ImageStored(id, store_image(folder, image).await)
                        });
                    }
                    None => {
                        self.log("Save the document first, then paste images".into());
                        let cancel = self.editor.execute(Command::CancelImage { id: image.id });
                        self.handle(cancel);
                    }
                },
                Event::ImageRejected { reason, .. } => {
                    let reason = reason.message(self.editor.current_language());
                    self.log(format!("Image not inserted: {reason}"))
                }
                Event::ImageRequested => perform(self.done, async {
                    Done::ImagePicked(pick("Images", &["png", "jpg", "jpeg", "gif", "webp"]).await)
                }),
                other => self.log(format!("{other:?}")),
            }
        }
    }

    /// Background work finished.
    fn finished(&mut self, done: Done) {
        match done {
            Done::ImagePicked(Some(path)) => {
                let output = self.editor.execute(Command::InsertImageFile(path));
                self.handle(output);
            }
            Done::ImagePicked(None) | Done::Opened(None) => {}
            Done::ImageStored(id, Ok(url)) => {
                self.log(format!("Image saved: {url}"));
                let output = self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                });
                self.handle(output);
            }
            Done::ImageStored(id, Err(error)) => {
                self.log(format!("Image not saved: {error}"));
                let output = self.editor.execute(Command::CancelImage { id });
                self.handle(output);
            }
            Done::Opened(Some(path)) => perform(self.done, async move {
                Done::Loaded(
                    std::fs::read_to_string(&path)
                        .map(|text| (path, text))
                        .map_err(|error| error.to_string()),
                )
            }),
            Done::Loaded(Ok((path, markdown))) => {
                self.editor.set_markdown(&markdown);
                self.log(format!("Opened {}", path.display()));
                self.path = Some(path);
                self.editor.set_image_base(self.folder());
            }
            Done::Loaded(Err(error)) => self.log(format!("Open failed: {error}")),
            Done::Saved(Ok(path)) => {
                self.editor.mark_saved();
                self.log(format!("Saved {}", path.display()));
                self.path = Some(path);
                self.editor.set_image_base(self.folder());
            }
            Done::Saved(Err(error)) => self.log(error),
        }
    }

    fn new_document(&mut self) {
        self.editor.set_markdown("");
        self.editor.set_image_base(None);
        self.path = None;
        let output = self.editor.execute(Command::Focus);
        self.handle(output);
    }

    fn open(&mut self) {
        perform(self.done, async {
            Done::Opened(pick("Markdown", &["md", "markdown", "txt"]).await)
        });
    }

    fn save(&mut self, save_as: bool) {
        let markdown = self.editor.markdown();
        let current = if save_as { None } else { self.path.clone() };
        perform(self.done, async move {
            let path = match current {
                Some(path) => Some(path),
                None => rfd::AsyncFileDialog::new()
                    .add_filter("Markdown", &["md"])
                    .set_file_name("document.md")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf()),
            };
            Done::Saved(match path {
                Some(path) => std::fs::write(&path, markdown)
                    .map(|_| path)
                    .map_err(|error| error.to_string()),
                None => Err("Save cancelled".to_string()),
            })
        });
    }

    fn summary(&self) -> String {
        let paper = self.editor.current_paper();
        let zoom = self.editor.current_zoom() * 100.0;
        if paper.is_fill() {
            return format!("Full width (no paper) \u{00B7} {zoom:.0} %");
        }
        format!(
            "{} {:?} \u{00B7} page {} of {} \u{00B7} {zoom:.0} %",
            paper.size,
            paper.orientation,
            self.editor.current_page(),
            self.editor.page_count(),
        )
    }
}
