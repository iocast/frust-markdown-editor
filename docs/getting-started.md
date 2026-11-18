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

That is all: the widget draws with the bundled DejaVu fonts (registered with Frust
on first use), blinks the caret, performs clipboard requests and receives dropped
files itself. There is no subscription or task to wire up.

`markdown()` builds the whole text: call it on `Event::Changed` or when
saving, not in `build`. See `examples/minimal.rs`.

Async work (file dialogs, uploads) in the examples uses `frust::spawn` and a
signal to bring the result back into the state; see `examples/common/mod.rs`.
