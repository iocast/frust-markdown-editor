# Getting started

```toml
[dependencies]
frust = { package = "frust-ui", version = "0.6" }
frust-markdown-editor = "0.3"
```

1. Keep an `EditorState` in your component state, e.g.
   `EditorState::with_markdown(text).paper(Paper::a4())`.
2. In `build`: `markdown_editor(&state.editor).view(|state: &mut State, action| { … })`.
   The closure receives every `Action` of the widget: pass it to
   `state.editor.update(action)` and handle the events of the returned `Output`.
   Commands you run with `execute(command)` return an `Output` too; handle it the same way.
3. Inside a `column()` or `row()`, add the editor with `.flex(1, …)` so it gets the
   remaining room (`.width(…)` / `.height(…)` take `Length::Fill` or a fixed size).
   `.height(Length::Content)` makes it as tall as the document instead, for a
   scrolling parent: the page itself then never scrolls vertically.

That is all: the widget draws with the bundled DejaVu fonts (registered with Frust
on first use), blinks the caret, performs clipboard requests and receives dropped
files itself. There is no subscription or task to wire up.

`markdown()` builds the whole text: call it on `Event::Changed` or when
saving, not in `build`. See `examples/minimal.rs`.

## Showing Markdown

To show a document rather than edit it, make it read-only and as tall as its content:

```rust
let viewer = EditorState::with_markdown(text)
    .paper(Paper::fill())
    .read_only(true);

// In `build`, inside a `scroll_view`:
markdown_editor(&state.viewer)
    .status_bar(false)
    .height(Length::Content)
    .view(|state: &mut State, action| {
        for event in state.viewer.update(action).events {
            if let Event::LinkClicked(url) = event {
                // open `url`
            }
        }
    })
```

A read-only editor has no toolbar, caret or on-screen keyboard. Text can be
selected and copied, a click follows a link, the checkboxes of task lists do not
react, and the context menu only offers copying. The wheel and finger drags go to
the scrolling parent while the document fits. `set_read_only(false)` makes it
editable again; `set_markdown` still replaces the document.

Async work (file dialogs, uploads) in the examples uses `frust::spawn` and a
signal to bring the result back into the state; see `examples/common/mod.rs`.
