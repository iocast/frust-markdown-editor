# API

| Item | Purpose |
|---|---|
| `EditorState` | Document, selection, history, settings |
| `markdown_editor(&state)` | The widget (`.toolbar(bool)`, `.status_bar(bool)`, `.context_menu(bool)`, `.width(…)`, `.height(…)`, `.view(\|state, action\| …)` → `EditorView`) |
| `Action`, `Output`, `Event`, `Command` | Message flow |
| `Paper`, `PageSize`, `Margins`, `Measure` | Page setup |
| `ZoomSettings`, `KeyBinding`, `Key`, `Named`, `Modifiers` | Zoom |
| `EditorStyle`, `Palette`, `Typography` | Theming |
| `ImageSettings`, `PastedImage`, `ImageRejection` | Images |
| `CodeHighlighter`, `SyntectHighlighter` | Code highlighting |
| `Language` | Interface language: English, German, French, Spanish, Italian (`.language(…)`) |
| `Features` | Switch tables, task lists, strikethrough on or off (`.features(…)`) |
| `BlockRenderer`, `RenderRequest` | Draw fenced code blocks of chosen languages, e.g. math or diagrams (`.block_renderer(…)`) |
| `normalize(&str)` | Markdown as the editor writes it |
| `Color`, `Point`, `Size`, `Rectangle`, `Vector`, `extension::Frame` | Colours, geometry and drawing for renderers and palettes |
| `FONTS`, `SANS_FAMILY`, `MONO_FAMILY` | The bundled DejaVu fonts (registered with Frust automatically) |

Full reference on docs.rs.
