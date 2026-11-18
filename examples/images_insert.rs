//! Use case: documents that reference existing image files or web images.
//! Nothing is copied, uploaded or embedded: the app inserts links.
//!
//! cargo run --example images_insert --features images

#[path = "common/mod.rs"]
mod common;

use common::{Inbox, action, bar, drain, inbox, perform};
use frust::{
    Component, DesktopConfig, EdgeInsets, Padding, SizedBox, View, any, column, text, text_input,
};
use frust_markdown_editor::{Command, EditorState, ImageSettings, Paper, markdown_editor};
use std::path::{Path, PathBuf};

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name("Insert image links")
        .with_initial_size((1100.0, 860.0));
    if let Err(error) = frust::run_desktop_config(App, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

struct App;

struct State {
    editor: EditorState,
    folder: PathBuf,
    url: String,
    alt: String,
    picked: Inbox<Option<PathBuf>>,
}

/// `path` relative to `base` when it is inside it, with `/` separators.
fn relative(path: &Path, base: &Path) -> String {
    let path = path.strip_prefix(base).unwrap_or(path);
    path.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let folder = std::env::current_dir().unwrap_or_default();
        State {
            editor: EditorState::with_markdown(
                "# Image links\n\nPut the caret where the image should go, then pick a \
                 file or enter a URL.\n",
            )
            .images(ImageSettings::disabled())
            .image_base(&folder)
            .paper(Paper::a4()),
            folder,
            url: String::new(),
            alt: String::new(),
            picked: inbox(),
        }
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        for path in drain(state.picked).into_iter().flatten() {
            let alt = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let url = relative(&path, &state.folder);
            state.editor.execute(Command::InsertImageLink { url, alt });
        }

        let top = bar(vec![
            any(action("Insert file…", |state: &mut State| {
                perform(state.picked, async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                })
            })),
            any(SizedBox(Some(320.0), None).child(
                text_input(state.url.clone(), |state: &mut State, url| state.url = url)
                    .placeholder("https://example.com/picture.png")
                    .on_submit(|state: &mut State, _| state.insert_url()),
            )),
            any(SizedBox(Some(180.0), None).child(
                text_input(state.alt.clone(), |state: &mut State, alt| state.alt = alt)
                    .placeholder("Description")
                    .on_submit(|state: &mut State, _| state.insert_url()),
            )),
            any(action("Insert URL", |state: &mut State| state.insert_url())),
        ]);

        let footer = Padding(
            EdgeInsets::all(6.0),
            text(format!("Links relative to {}", state.folder.display())).size(12.0),
        );

        column()
            .child(top)
            .flex(
                1,
                markdown_editor(&state.editor).view(|state: &mut State, action| {
                    state.editor.update(action);
                }),
            )
            .child(footer)
    }
}

impl State {
    fn insert_url(&mut self) {
        let url = std::mem::take(&mut self.url);
        let alt = std::mem::take(&mut self.alt);
        if url.trim().is_empty() {
            return;
        }
        self.editor.execute(Command::InsertImageLink {
            url: url.trim().to_string(),
            alt,
        });
    }
}
