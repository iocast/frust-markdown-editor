//! Use case: a desktop notes app. Images are saved as files in an `assets/`
//! folder next to the document; the Markdown links them relatively.
//!
//! cargo run --example images_folder --features images

#[path = "common/mod.rs"]
mod common;

use common::{Inbox, action, bar, drain, inbox, perform};
use frust::{Component, DesktopConfig, View, any, column, text};
use frust_markdown_editor::{
    Command, EditorState, Event, ImageId, ImageSettings, Output, Paper, PastedImage,
    markdown_editor,
};
use std::path::PathBuf;

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name("Notes with image folder")
        .with_initial_size((1100.0, 860.0));
    if let Err(error) = frust::run_desktop_config(App, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

struct App;

struct State {
    editor: EditorState,
    /// Folder of the note; images go to `<folder>/assets`.
    folder: PathBuf,
    status: String,
    done: Inbox<Done>,
}

enum Done {
    Stored(ImageId, Result<String, String>),
    Picked(Option<PathBuf>),
}

/// Saves the image and returns the URL relative to the note.
async fn store(folder: PathBuf, image: PastedImage) -> Result<String, String> {
    let original = image.file_name();
    let (stem, extension) = original
        .rsplit_once('.')
        .unwrap_or((original.as_str(), image.extension()));
    let mut name = original.clone();
    let mut counter = 1;
    while folder.join("assets").join(&name).exists() {
        name = format!("{stem}-{counter}.{extension}");
        counter += 1;
    }
    std::fs::write(folder.join("assets").join(&name), &*image.bytes)
        .map_err(|error| error.to_string())?;
    Ok(format!("assets/{name}"))
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let folder = std::env::current_dir()
            .unwrap_or_default()
            .join("example-notes");
        let _ = std::fs::create_dir_all(folder.join("assets"));
        let note = std::fs::read_to_string(folder.join("note.md")).unwrap_or_else(|_| {
            "# My note\n\nPaste a screenshot (Ctrl+V), drop an image file onto the window, \
             or use the **Image** button. Images are saved in `example-notes/assets/`.\n"
                .to_string()
        });

        let editor = EditorState::with_markdown(&note)
            .images(ImageSettings::delegate().max_bytes(20 * 1024 * 1024))
            .image_base(&folder)
            .paper(Paper::a4());

        State {
            editor,
            status: format!("Folder: {}", folder.display()),
            folder,
            done: inbox(),
        }
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        for done in drain(state.done) {
            state.finished(done);
        }

        let top = bar(vec![
            any(action("Save note.md", |state: &mut State| state.save())),
            any(text(state.status.clone()).size(12.0)),
        ]);
        column().child(top).flex(
            1,
            markdown_editor(&state.editor).view(|state: &mut State, action| {
                let output = state.editor.update(action);
                state.handle(output);
            }),
        )
    }
}

impl State {
    fn handle(&mut self, output: Output) {
        for event in output.events {
            match event {
                Event::ImagePasted(image) => {
                    self.status = format!("Saving {}…", image.file_name());
                    let id = image.id;
                    let folder = self.folder.clone();
                    perform(self.done, async move {
                        Done::Stored(id, store(folder, image).await)
                    });
                }
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Not inserted: {reason}");
                }
                Event::ImageRequested => perform(self.done, async {
                    Done::Picked(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf()),
                    )
                }),
                _ => {}
            }
        }
    }

    fn finished(&mut self, done: Done) {
        let output = match done {
            Done::Stored(id, Ok(url)) => {
                self.status = format!("Saved {url}");
                self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                })
            }
            Done::Stored(id, Err(error)) => {
                self.status = format!("Could not save the image: {error}");
                self.editor.execute(Command::CancelImage { id })
            }
            Done::Picked(Some(path)) => self.editor.execute(Command::InsertImageFile(path)),
            Done::Picked(None) => return,
        };
        self.handle(output);
    }

    fn save(&mut self) {
        let path = self.folder.join("note.md");
        match std::fs::write(&path, self.editor.markdown()) {
            Ok(()) => {
                self.editor.mark_saved();
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }
}
