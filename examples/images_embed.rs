//! Use case: single-file documents. Images are written into the Markdown as
//! `data:` URLs, so the `.md` file contains everything. Limited to 512 KiB.
//!
//! * `ImageSettings::embed()` – no application code needed for storage.
//! * `Event::ImageRequested` – the toolbar "Image" button: show a file dialog
//!   and send `Command::InsertImageFile`.
//! * `Event::ImageRejected` – tell the user why an image was not inserted.
//! * The preview is rebuilt on `Event::Changed`, not in `build`.
//!
//! cargo run --example images_embed --features images

#[path = "common/mod.rs"]
mod common;

use common::{Inbox, drain, inbox, perform};
use frust::{
    Component, CrossAxisAlignment, DesktopConfig, EdgeInsets, Padding, SizedBox, View, column, row,
    scroll_view, text,
};
use frust_markdown_editor::{
    Command, EditorState, Event, ImageSettings, Output, Paper, markdown_editor,
};
use std::path::PathBuf;

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name("Embedded images")
        .with_initial_size((1300.0, 820.0));
    if let Err(error) = frust::run_desktop_config(App, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

struct App;

struct State {
    editor: EditorState,
    status: String,
    /// Shortened Markdown for the side panel, rebuilt only on changes.
    preview: String,
    size: usize,
    picked: Inbox<Option<PathBuf>>,
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let editor = EditorState::with_markdown(
            "# Embedded images\n\nPaste, drop or insert an image (up to 512 KiB). It is \
             stored inside the Markdown as a `data:` URL.\n",
        )
        .images(ImageSettings::embed().max_bytes(512 * 1024).accept([
            "image/png",
            "image/jpeg",
            "image/gif",
        ]))
        .paper(Paper::fill());

        let mut state = State {
            editor,
            status: String::new(),
            preview: String::new(),
            size: 0,
            picked: inbox(),
        };
        state.refresh();
        state
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        for path in drain(state.picked).into_iter().flatten() {
            let output = state.editor.execute(Command::InsertImageFile(path));
            state.handle(output);
        }

        let side = SizedBox(Some(420.0), None).child(Padding(
            EdgeInsets::all(12.0),
            scroll_view(
                column()
                    .cross_axis(CrossAxisAlignment::Start)
                    .child(text(format!("Markdown: {} bytes", state.size)).size(12.0))
                    .child(text(state.status.clone()).size(12.0))
                    .child(text(state.preview.clone()).size(12.0)),
            ),
        ));

        let editor = markdown_editor(&state.editor).view(|state: &mut State, action| {
            let output = state.editor.update(action);
            state.handle(output);
        });
        row().flex(1, editor).child(side)
    }
}

impl State {
    /// Builds the preview once per change.
    fn refresh(&mut self) {
        let markdown = self.editor.markdown();
        self.size = markdown.len();
        self.preview = markdown
            .lines()
            .map(|line| {
                let count = line.chars().count();
                if count > 120 {
                    let short: String = line.chars().take(120).collect();
                    format!("{short}… ({count} characters)")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    /// Reacts to the events of every editor output.
    fn handle(&mut self, output: Output) {
        for event in output.events {
            match event {
                Event::ImageRejected { source, reason } => {
                    self.status = format!("{source:?}: {reason}");
                }
                Event::ImageRequested => perform(self.picked, async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "gif"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                }),
                Event::Changed => {
                    self.status.clear();
                    self.refresh();
                }
                _ => {}
            }
        }
    }
}
