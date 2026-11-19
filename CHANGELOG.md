# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

## [0.4.0] - 2026-11-19

### Added
- Read-only documents: `EditorState::read_only` / `set_read_only`, to show Markdown rather than edit it. No toolbar, caret or on-screen keyboard; text can be selected and copied, a click follows a link and the context menu only offers copying (needs `markdown-editor-core` with read-only support)
- `Length::Content`: the editor is as tall as the document and the bars, for a scrolling parent

### Changed
- The mouse wheel goes to the parent when the document fits the page, so a scrolling parent scrolls

## [0.3.2] - 2026-11-18

### Fixed
- Dragging a selection to the edge of the view now scrolls along (with `markdown-editor-core` 0.1.1 or newer)

## [0.3.1] - 2026-11-18

### Changed
- **Breaking:** the document, Markdown, layout and editing logic moved to the new `markdown-editor-core` crate, which is shared with `iced-markdown-editor`. `EditorState` dereferences to its `EditorCore`
- **Breaking:** `EditorStyle::from_theme` needs the `FromTheme` trait in scope, and `ImageRejection::message` needs `ImageRejectionMessage`

### Fixed
- A double click sometimes selected only part of a word, when the mouse moved slightly during the click; dragging after a double or triple click now extends the selection by whole words or paragraphs

[unreleased]: https://github.com/iocast/frust-markdown-editor/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/iocast/frust-markdown-editor/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/iocast/frust-markdown-editor/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/iocast/frust-markdown-editor/releases/tag/v0.3.1
