//! The editor in the browser. Also runs on the desktop with `cargo run`.
//!
//! * full width editor with toolbar and status bar
//! * syntax highlighting for code blocks (`syntect` feature)
//! * design system switcher (Material 3, shadcn/ui, Cupertino, beUI, Glyph,
//!   plain Frust), light / dark, paper / full width
//! * the live Markdown in a side panel (updated on `Event::Changed`)
//! * images embedded as `data:` URLs; the **Image** button opens the
//!   browser's file picker and inserts the file with `Command::InsertImage`
//! * phones and narrow windows: the top bar takes two rows, the Markdown
//!   panel starts hidden and goes below the editor (the editor itself
//!   adapts its toolbar and handles touch)

use frust::{
    AnyView, Brightness, ButtonStyle, Component, CrossAxisAlignment, EdgeInsets, OverlayPlacement,
    OverlaySide, Padding, RwSignal, SizedBox, Theme, Update, View, WindowMetrics, With, any,
    column, overlay_portal, row, scroll_view, text, use_context,
};
use frust_markdown_editor::{
    Command, EditorState, EditorStyle, Event, FromTheme, ImageSettings, Output, Paper,
    SyntectHighlighter, markdown_editor,
};
use std::fmt;

/// Banner image (`banner.png`), embedded so the demo needs no server.
const BANNER_BASE64: &str = include_str!("../banner.b64");

/// Windows narrower than this (phones, a narrow browser window) get the
/// compact layout.
const NARROW: f64 = 720.0;

/// Whether the window is narrow. Frust provides the window size and
/// rebuilds when it changes.
fn is_narrow() -> bool {
    use_context::<WindowMetrics>().is_some_and(|metrics| metrics.size.width < NARROW)
}

fn sample() -> String {
    let banner = format!("data:image/png;base64,{}", BANNER_BASE64.trim());
    let lines = [
        "# frust-markdown-editor in the browser",
        "",
        "A Word-style **WYSIWYG** editor for *CommonMark*, written in Rust with",
        "[Frust](https://frust.dev). This page runs it as WebAssembly.",
        "",
        "![frust-markdown-editor](BANNER)",
        "",
        "## Try it",
        "",
        "- [x] Type, select, use **Ctrl+B**, *Ctrl+I*, `Ctrl+E`",
        "- [ ] Press **Tab** at the start of an item to nest it",
        "  - [ ] Click a checkbox",
        "- [ ] Switch to **Source** in the toolbar",
        "- [ ] Insert an image with the **Image** button",
        "",
        "> Block quotes, numbered lists and code blocks work too.",
        "",
        "1. First",
        "2. Second",
        "",
        "```rust",
        "use frust_markdown_editor::{EditorState, markdown_editor};",
        "",
        "fn build(&self, state: &mut State) -> impl View<State> {",
        "    markdown_editor(&state.editor).view(|state: &mut State, action| {",
        "        state.editor.update(action);",
        "    })",
        "}",
        "```",
    ];
    lines.join("\n").replace("BANNER", &banner) + "\n"
}

/// The Frust design systems the demo can switch between.
#[derive(Clone, Copy, PartialEq)]
enum Design {
    Material,
    Shadcn,
    Cupertino,
    Beui,
    Glyph,
    Frust,
}

impl Design {
    const ALL: [Design; 6] = [
        Design::Material,
        Design::Shadcn,
        Design::Cupertino,
        Design::Beui,
        Design::Glyph,
        Design::Frust,
    ];

    fn theme(self, brightness: Brightness) -> Theme {
        let theme = match self {
            Design::Material => frust_material::baseline(),
            Design::Shadcn => frust_shadcn::theme(),
            Design::Cupertino => frust_cupertino::baseline(),
            Design::Beui => frust_beui::theme(),
            Design::Glyph => frust_glyph::baseline(),
            Design::Frust => Theme::neutral(),
        };
        theme.with_brightness(brightness)
    }
}

impl fmt::Display for Design {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Design::Material => "Material 3",
            Design::Shadcn => "shadcn/ui",
            Design::Cupertino => "Cupertino",
            Design::Beui => "beUI",
            Design::Glyph => "Glyph",
            Design::Frust => "Frust",
        })
    }
}

#[derive(Default)]
struct App;

struct State {
    editor: EditorState,
    design: Design,
    /// The active theme (design system + brightness).
    theme: Theme,
    /// Whether the design system list is open.
    picking: bool,
    dark: bool,
    paper: bool,
    /// Whether the Markdown panel is shown; `None`: only on wide windows.
    preview: Option<bool>,
    /// Markdown for the side panel, rebuilt on changes only.
    markdown: String,
    status: String,
    /// Files picked in the browser's file dialog.
    picked: RwSignal<Vec<(String, Vec<u8>)>>,
}

/// Shortens long lines (embedded images) for the side panel.
fn shorten(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let count = line.chars().count();
            if count > 100 {
                let short: String = line.chars().take(100).collect();
                format!("{short}… ({count} characters)")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A toggle button for the top bar, drawn with the design system's own widget.
fn toggle(
    design: Design,
    name: &str,
    on: bool,
    press: impl Fn(&mut State) + 'static,
) -> AnyView<State> {
    match design {
        Design::Material => any(frust_material::tonal_toggle_button(
            on,
            move |state: &mut State, _| press(state),
        )
        .label(name)),
        Design::Shadcn => any(frust_shadcn::button(name, press)
            .variant(if on {
                frust_shadcn::ButtonVariant::Default
            } else {
                frust_shadcn::ButtonVariant::Outline
            })
            .size(frust_shadcn::ButtonSize::Sm)),
        Design::Cupertino => any(frust_cupertino::cupertino_button(name, press)
            .style(if on {
                frust_cupertino::CupertinoButtonStyle::Filled
            } else {
                frust_cupertino::CupertinoButtonStyle::Gray
            })
            .size(frust_cupertino::CupertinoButtonSize::Small)),
        Design::Beui => any(frust_beui::components::button::button(name, press)
            .tone(if on {
                frust_beui::components::button::ButtonTone::Primary
            } else {
                frust_beui::components::button::ButtonTone::Outline
            })
            .size(frust_beui::components::button::ButtonSize::Sm)),
        // Glyph has no plain button: a label next to a Glyph switch.
        Design::Glyph => any(row()
            .cross_axis(CrossAxisAlignment::Center)
            .child(text(name.to_string()).size(13.0))
            .child(SizedBox(Some(6.0), None))
            .child(frust_glyph::toggle(on, move |state: &mut State, _| press(state)).label(name))),
        Design::Frust => any(frust::button(name.to_string(), press)
            .style(if on {
                ButtonStyle::Primary
            } else {
                ButtonStyle::Secondary
            })
            .small()),
    }
}

/// The design system switcher: a button that opens a list of all systems.
fn design_picker(state: &State) -> impl View<State> {
    let design = state.design;
    let trigger = toggle(
        design,
        &format!("{design} \u{25BE}"),
        state.picking,
        |state: &mut State| state.picking = !state.picking,
    );
    let list = state.picking.then(|| {
        let mut list = column::<State>();
        for option in Design::ALL {
            list = list.child(Padding(
                EdgeInsets::all(3.0),
                toggle(
                    design,
                    &option.to_string(),
                    option == design,
                    move |state: &mut State| {
                        state.picking = false;
                        state.set_theme(option, state.dark);
                    },
                ),
            ));
        }
        let scheme = state.theme.scheme();
        frust::container(Padding(EdgeInsets::all(4.0), list))
            .fill(scheme.surface_container_high)
            .border(scheme.outline_variant, 1.0)
            .radius(8.0)
    });
    overlay_portal(trigger)
        .overlay(list)
        .placement(OverlayPlacement::on(OverlaySide::Bottom))
        .on_outside_tap(|state: &mut State| state.picking = false)
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let editor = EditorState::with_markdown(&sample())
            .images(ImageSettings::embed().max_bytes(2 * 1024 * 1024))
            .highlighter(SyntectHighlighter::new())
            .paper(Paper::fill())
            .tab_width(2);
        let markdown = shorten(&editor.markdown());
        let mut state = State {
            editor,
            design: Design::Material,
            theme: Theme::neutral(),
            picking: false,
            dark: false,
            paper: false,
            preview: None,
            markdown,
            status: String::new(),
            picked: RwSignal::new(Vec::new()),
        };
        state.set_theme(Design::Material, false);
        state
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        // Files picked in the dialog (delivered by `pick_image`).
        if state.picked.with(|files| !files.is_empty()) {
            let mut files = Vec::new();
            state.picked.update(|picked| files = std::mem::take(picked));
            for (name, bytes) in files {
                let output = state.editor.execute(Command::InsertImage {
                    bytes,
                    name: Some(name),
                });
                state.handle(output);
            }
        }

        let design = state.design;
        let narrow = is_narrow();
        let preview = state.preview.unwrap_or(!narrow);
        let toggles = row()
            .cross_axis(CrossAxisAlignment::Center)
            .child(toggle(design, "Dark", state.dark, |state: &mut State| {
                state.set_theme(state.design, !state.dark);
            }))
            .child(SizedBox(Some(8.0), None))
            .child(toggle(
                design,
                if narrow { "A4" } else { "A4 paper" },
                state.paper,
                move |state: &mut State| {
                    state.paper = !state.paper;
                    state.editor.set_paper(if state.paper {
                        Paper::a4()
                    } else {
                        Paper::fill()
                    });
                    if state.paper && narrow {
                        // The whole page width on a phone screen.
                        let output = state.editor.execute(Command::FitWidth);
                        state.handle(output);
                    }
                },
            ))
            .child(SizedBox(Some(8.0), None))
            .child(toggle(
                design,
                "Markdown",
                preview,
                move |state: &mut State| {
                    state.preview = Some(!preview);
                },
            ));
        let status = text(state.status.clone()).size(12.0);
        // Phones: the design system and status above, the toggles below.
        let bar: AnyView<State> = if narrow {
            any(Padding(
                EdgeInsets::symmetric(8.0, 4.0),
                column()
                    .child(
                        row()
                            .cross_axis(CrossAxisAlignment::Center)
                            .child(design_picker(state))
                            .child(SizedBox(Some(8.0), None))
                            .flex(1, status),
                    )
                    .child(SizedBox(None, Some(4.0)))
                    .child(toggles),
            ))
        } else {
            any(Padding(
                EdgeInsets::symmetric(10.0, 6.0),
                row()
                    .cross_axis(CrossAxisAlignment::Center)
                    .child(text("frust-markdown-editor").size(15.0))
                    .child(SizedBox(Some(12.0), None))
                    .flex(1, status)
                    .child(design_picker(state))
                    .child(SizedBox(Some(8.0), None))
                    .child(toggles),
            ))
        };

        let editor = markdown_editor(&state.editor).view(|state: &mut State, action| {
            let output = state.editor.update(action);
            state.handle(output);
        });

        let side = || {
            Padding(
                EdgeInsets::all(12.0),
                scroll_view(text(state.markdown.clone()).size(12.0)),
            )
        };
        // Beside the editor, or below it on a phone.
        let body: AnyView<State> = match (preview, narrow) {
            (true, false) => any(row().flex(3, editor).flex(2, side())),
            (true, true) => any(column().flex(3, editor).flex(2, side())),
            (false, _) => any(row().flex(1, editor)),
        };

        column().child(bar).flex(1, body)
    }
}

impl State {
    /// Switches the editor and the rest of the page (bar, side panel).
    fn set_theme(&mut self, design: Design, dark: bool) {
        self.design = design;
        self.dark = dark;
        let brightness = if dark {
            Brightness::Dark
        } else {
            Brightness::Light
        };
        self.theme = design.theme(brightness);
        self.editor.set_style(EditorStyle::from_theme(&self.theme));
        frust::set_app_theme(self.theme.clone());
    }

    fn handle(&mut self, output: Output) {
        for event in output.events {
            match event {
                Event::Changed => {
                    self.markdown = shorten(&self.editor.markdown());
                    self.status.clear();
                }
                Event::ImageRequested => pick_image(self.picked),
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Image not inserted: {reason}");
                }
                _ => {}
            }
        }
    }
}

/// Opens the file dialog (the browser's file picker on the web).
fn pick_image(picked: RwSignal<Vec<(String, Vec<u8>)>>) {
    // `rfd`'s handles are not `Send` in the browser: run on the UI thread.
    frust::spawn_local(async move {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
            .pick_file()
            .await
        else {
            return;
        };
        let bytes = file.read().await;
        picked.update(|files| files.push((file.file_name(), bytes)));
    });
}

// The browser entry point (wasm32) and `__frust_main` for the desktop.
frust::app!(
    App,
    setup = {
        // Registers each design system's fonts; must run before the first
        // frame. The theme itself is switched at runtime (`State::set_theme`).
        frust_shadcn::install();
        frust_cupertino::install();
        frust_beui::install();
        frust_glyph::install();
        frust_material::install();
    },
    desktop = {
        frust::DesktopConfig::new()
            .with_app_name("frust-markdown-editor · web demo")
            .with_initial_size((1200.0, 820.0))
    }
);

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    __frust_main();
}
