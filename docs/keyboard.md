# Keyboard and mouse

| Keys | Action |
|---|---|
| `Ctrl+B` / `I` / `D` / `E` | Bold / italic / strikethrough / code |
| `Ctrl+K` | Link |
| `Ctrl+1` / `2` / `3`, `Ctrl+Shift+N` | Heading, normal |
| `Ctrl+U` / `Ctrl+L` / `Ctrl+Shift+L` | Bullet / numbered / task list |
| `Ctrl+Enter` | Check / uncheck task |
| `Ctrl+Shift+Q` | Block quote |
| `Tab` / `Shift+Tab` | Nest / un-nest list items, indent / outdent |
| `Tab` / `Shift+Tab` in a table | Next / previous cell; `Tab` in the last cell adds a row |
| `Enter` in a table | Cell below; in the last row a new row, on an empty last row leave the table |
| `Ctrl+Z`, `Ctrl+Y` | Undo, redo |
| `Ctrl` + `+` / `-` / `0`, `Ctrl` + wheel | Zoom |
| Wheel, `Shift` + wheel | Scroll vertically / horizontally |
| Drag the scrollbars | Scroll |
| Right-click | Context menu: cut, copy, paste and formatting for a selection; insert / delete rows and columns and align columns in a table |
| Link dialog (context menu) | `Enter` OK, `Escape` cancel, `Tab` next field |
| Double / triple click | Select word / paragraph |
| `Shift` + click | Extend the selection (Windows, macOS) |
| `Ctrl` + click, middle click | Follow a link (`Event::LinkClicked`; `Ctrl` + click on Windows and macOS) |

## Touch screens

| Gesture | Action |
|---|---|
| Tap | Place the caret; opens the on-screen keyboard |
| Double / triple tap | Select word / paragraph |
| Drag | Scroll |
| Long press | Select the word and open the context menu |
| Long press, then drag | Select text |
| Two fingers apart / together | Zoom (`ZoomSettings::mouse_wheel`) |
| Swipe the toolbar | Scroll it sideways when it is too wide for one row |

After a touch the toolbar, the status bar and the context menu get larger
targets; a mouse click brings the usual sizes back. Narrow editors show the
toolbar as one row that scrolls sideways, and the status bar leaves out the
labels it has no room for. Text composed by an input method (whole words on
Android keyboards, CJK) appears at the caret while it is composed.

In the browser the host page should give the canvas `touch-action: none`, so
the browser leaves the gestures to the editor; `examples/web/index.html` also
keeps the editor above the on-screen keyboard.

## Notes

Frust 0.6 does not report modifier keys with mouse events; the editor asks the
operating system, which works on Windows and macOS. The Frust desktop and web
shells do not deliver `PageUp` / `PageDown`, so those keys have no effect.

Type `| Name | Age |` and press Enter to turn the line into a table.

Source mode: Enter continues lists and quotes, Tab nests items, the toolbar
edits the Markdown text of the selected lines.
