# Web example

The editor in the browser (WebAssembly; WebGPU with a WebGL2 fallback).

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk          # or: cargo binstall trunk
cd examples/web
trunk serve --open           # http://127.0.0.1:8080
trunk build --release        # static files in dist/
```

`cargo run` in this folder runs the same app on the desktop.

`index.html` loads `frust_web.js`, Frust's host-page glue (copied from
`frust-shell-web/platform/web`), which keeps the canvas sized to the page.

In the browser there is no file system and no desktop clipboard: pasting
images, dropping files and `image_base` are desktop features. Images are
embedded as `data:` URLs; the **Image** button uses the browser's file picker.

On a phone or tablet the editor scrolls with a finger, zooms with two, and
selects with a long press; the page keeps it above the on-screen keyboard.
Below 720 pixels the top bar takes two rows and the Markdown panel starts
hidden.

Frust 0.6.6 or later is needed: 0.6.3 panicked at start in the browser
("time not implemented on this platform"). Keep all Frust crates at the same
version; see `docs/web.md`.
