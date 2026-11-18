//! Use case: a web-backed or team app. Images are uploaded to a server and
//! the Markdown links the returned URL. The server is simulated: an upload
//! waits 1.5 s, every third upload fails.
//!
//! cargo run --example images_upload --features images

#[path = "common/mod.rs"]
mod common;

use common::{Inbox, action_if, bar, drain, inbox, perform};
use frust::{Component, DesktopConfig, View, any, column, text};
use frust_markdown_editor::{
    Command, EditorState, Event, ImageId, ImageSettings, Output, PastedImage, markdown_editor,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SERVER: &str = "https://uploads.example.com/";

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name("Upload images")
        .with_initial_size((1100.0, 860.0));
    if let Err(error) = frust::run_desktop_config(App, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

/// Simulated image server: URL → bytes.
type Server = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct App;

struct State {
    editor: EditorState,
    server: Server,
    uploads: u32,
    status: String,
    done: Inbox<Done>,
}

enum Done {
    Uploaded(ImageId, Result<String, String>),
    Picked(Option<PathBuf>),
}

async fn upload(server: Server, image: PastedImage, attempt: u32) -> Result<String, String> {
    tokio::time::sleep(Duration::from_millis(1500)).await;
    if attempt.is_multiple_of(3) {
        return Err("server busy (simulated)".into());
    }
    let url = format!("{SERVER}{}", image.file_name());
    server
        .lock()
        .map_err(|_| "server error".to_string())?
        .insert(url.clone(), image.bytes.to_vec());
    Ok(url)
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let server: Server = Arc::default();
        let store = server.clone();
        let editor = EditorState::with_markdown(
            "# Uploads\n\nPaste, drop or insert images. They are uploaded (1.5 s, every \
             third upload fails) and linked by URL.\n",
        )
        .images(ImageSettings::delegate().max_bytes(5 * 1024 * 1024))
        .image_loader(move |url| store.lock().ok()?.get(url).cloned());

        State {
            editor,
            server,
            uploads: 0,
            status: String::new(),
            done: inbox(),
        }
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        for done in drain(state.done) {
            state.finished(done);
        }

        let pending = state.editor.pending_images();
        let top = bar(vec![
            any(action_if("Publish", pending == 0, |state: &mut State| {
                state.status = format!(
                    "Published {} bytes of Markdown",
                    state.editor.markdown().len()
                );
            })),
            any(text(if pending > 0 {
                format!("Uploading {pending} image(s)…")
            } else {
                String::new()
            })
            .size(12.0)),
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
                    self.uploads += 1;
                    let id = image.id;
                    let server = self.server.clone();
                    let attempt = self.uploads;
                    perform(self.done, async move {
                        Done::Uploaded(id, upload(server, image, attempt).await)
                    });
                }
                Event::ImageRejected { reason, .. } => {
                    self.status = format!("Not uploaded: {reason}");
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
            Done::Uploaded(id, Ok(url)) => {
                self.status = format!("Uploaded {url}");
                self.editor.execute(Command::ResolveImage {
                    id,
                    url,
                    alt: String::new(),
                })
            }
            Done::Uploaded(id, Err(error)) => {
                self.status = format!("Upload failed: {error}. The placeholder was removed.");
                self.editor.execute(Command::CancelImage { id })
            }
            Done::Picked(Some(path)) => self.editor.execute(Command::InsertImageFile(path)),
            Done::Picked(None) => return,
        };
        self.handle(output);
    }
}
