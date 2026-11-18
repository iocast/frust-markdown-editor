# Web

The crate builds for `wasm32-unknown-unknown` without extra features;
dependencies are chosen per target (`arboard`, `png` only on the desktop).
`web-time` replaces `std::time::Instant`.

Your web app (a binary that calls `frust::app!`):

```toml
[dependencies]
frust = { package = "frust-ui", version = "0.6.6" }
frust-markdown-editor = { version = "0.3", features = ["images"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"   # used by the entry point `frust::app!` generates
```

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
cd examples/web && trunk serve --open
```

`examples/web/index.html` includes Frust's host-page glue (`frust_web.js`, from
`frust-shell-web/platform/web`), which keeps the canvas sized to the page.

Not available in the browser: clipboard images, file drops, local files
(`image_base`, `InsertImageFile`). Use `embed()` or upload the bytes.

Phones and tablets: give the canvas `touch-action: none` in the host page, so
the browser leaves scrolling, selecting and pinch zoom to the editor (see
`examples/web/index.html`, which also keeps the editor above the on-screen
keyboard). The gestures are listed in [keyboard.md](keyboard.md).

Use Frust 0.6.6 or later in the browser, with all Frust crates (including the
design systems such as `frust-material`) at the same version. Frust 0.6.3
panicked at start there ("time not implemented on this platform": it read
`std::time::Instant`, which does not exist on `wasm32-unknown-unknown`).
