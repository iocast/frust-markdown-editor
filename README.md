# frust-markdown-editor

A Word-style WYSIWYG **CommonMark** editor widget for [Frust](https://frust.dev) 0.6,
with the GFM extensions strikethrough, task lists and tables. Desktop and web.

- Pages like a word processor (A4, Letter, custom sizes, margins) or full width
- Formatting toolbar, keyboard shortcuts, Word-style autoformat
- Nested lists, task lists with clickable checkboxes, block quotes, code blocks
- Tables with column alignment; Tab and Enter move between cells
- WYSIWYG and Markdown source mode on the same pages
- Zoom, undo / redo, clipboard
- Toolbar and status bar in English, German, French, Spanish and Italian (Fluent)
- Optional syntax highlighting (`syntect`) and images (`images`)
- Round trip tested against all 652 examples of the CommonMark 0.31.2 spec

```toml
[dependencies]
frust = { package = "frust-ui", version = "0.6" }
frust-markdown-editor = { version = "0.3", features = ["syntect", "images"] }
```

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

The widget draws the page, toolbar and status bar itself (with the bundled
DejaVu fonts) and handles caret blinking, the clipboard and dropped files. Inside
a Frust `column()` / `row()`, add it with `.flex(1, …)` so it fills the rest.

## Examples

```bash
cargo run --example editor --features syntect,images   # full demo
cargo run --example minimal
cargo run --example images_folder --features images
cargo run --example images_embed --features images
cargo run --example images_upload --features images
cargo run --example images_insert --features images
cd examples/web && trunk serve --open                  # in the browser
```

### Crash on start on Windows

If an example exits right after opening its window with
`exit code: 0xc0000005, STATUS_ACCESS_VIOLATION`, the crash is in the GPU's
Vulkan backend, not in the editor. It has been seen with Intel UHD Graphics.
Windows tries Vulkan first, so force DirectX 12 instead:

```powershell
$env:WGPU_BACKEND = "dx12"   # PowerShell, current session
cargo run --example minimal
```

```bash
WGPU_BACKEND=dx12 cargo run --example minimal   # Git Bash
```

To make it permanent, set `WGPU_BACKEND=dx12` as a user environment variable.
A newer graphics driver may also fix it. Frust has no setting to choose the
backend from code, so the variable is the only option. The
`Registry lookup failed to get layer manifest files` warning in the log is
harmless.

## Documentation

See [`docs/`](docs/README.md) and [docs.rs](https://docs.rs/frust-markdown-editor). To preview them locally, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE.txt) or [MIT license](LICENSE-MIT.txt) at your option. The bundled DejaVu fonts have their own license (`assets/fonts/LICENSE-DejaVu.txt`).
