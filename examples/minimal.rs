//! Minimal integration: the editor on the left, its live Markdown on the
//! right. Full width (no paper), zoom disabled, Tab = 2 spaces.
//!
//! cargo run --example minimal

use frust::{
    Component, CrossAxisAlignment, DesktopConfig, EdgeInsets, Padding, SizedBox, View, column, row,
    scroll_view, text,
};
use frust_markdown_editor::{EditorState, Event, Paper, ZoomSettings, markdown_editor};

pub fn main() {
    let config = DesktopConfig::new()
        .with_app_name("Minimal editor")
        .with_initial_size((1300.0, 820.0));
    if let Err(error) = frust::run_desktop_config(App, config) {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

struct App;

struct State {
    editor: EditorState,
    /// Updated on `Event::Changed`, not in `build`.
    markdown: String,
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        let editor = EditorState::with_markdown(
            "# Hello\n\nType here. Try **bold** with Ctrl+B.\n\n- [ ] click the box\n- [x] done\n  - nested\n",
        )
        .paper(Paper::fill())
        .zoom_settings(ZoomSettings::disabled())
        .tab_width(2);
        let markdown = editor.markdown();
        State { editor, markdown }
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        let editor =
            markdown_editor(&state.editor)
                .status_bar(false)
                .view(|state: &mut State, action| {
                    let output = state.editor.update(action);
                    if output.events.contains(&Event::Changed) {
                        state.markdown = state.editor.markdown();
                    }
                });

        let preview = SizedBox(Some(420.0), None).child(Padding(
            EdgeInsets::all(16.0),
            column()
                .cross_axis(CrossAxisAlignment::Start)
                .child(text("Markdown").size(12.0))
                .child(SizedBox(None, Some(8.0)))
                .flex(1, scroll_view(text(state.markdown.clone()).size(13.0))),
        ));

        row().flex(1, editor).child(preview)
    }
}
