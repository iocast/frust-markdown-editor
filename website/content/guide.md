+++
title = "Getting started"
template = "page.html"
+++

```toml
[dependencies]
frust = { package = "frust-ui", version = "0.6" }
frust-markdown-editor = { version = "0.3", features = ["syntect", "images"] }
```

1. Keep an `EditorState` in your component state, e.g.
   `EditorState::with_markdown(text).paper(Paper::a4())`.
2. Show it in `build` with `markdown_editor(&state.editor).view(|state: &mut State, action| { … })`.
   The closure gets every `Action`: pass it to `state.editor.update(action)` and
   handle the events of the returned `Output`.
3. Inside a `column()` or `row()`, add the editor with `.flex(1, …)` so it fills the rest.

The widget registers its bundled fonts, blinks the caret and handles the clipboard
and dropped files itself. There is no subscription or task to wire up.

```rust
use frust::{Component, View};
use frust_markdown_editor::{EditorState, Event, markdown_editor};

#[derive(Default)]
struct App;

struct State {
    editor: EditorState,
}

impl Component for App {
    type State = State;

    fn init(&self) -> State {
        State {
            editor: EditorState::with_markdown("# Hello\n\n- [ ] first task\n"),
        }
    }

    fn build(&self, state: &mut State) -> impl View<State> {
        markdown_editor(&state.editor).view(|state: &mut State, action| {
            let output = state.editor.update(action);
            if output.events.contains(&Event::Changed) {
                let _markdown = state.editor.markdown();
            }
        })
    }
}

fn main() {
    if let Err(error) = frust::run(App) {
        eprintln!("{error:?}");
    }
}
```

See the [examples](https://github.com/iocast/frust-markdown-editor/tree/main/examples)
and the [API documentation](https://docs.rs/frust-markdown-editor).
