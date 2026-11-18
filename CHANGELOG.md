# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

## [0.3.1] - 2026-11-18

### Changed
- **Breaking:** the document, Markdown, layout and editing logic moved to the new `markdown-editor-core` crate, which is shared with `iced-markdown-editor`. `EditorState` dereferences to its `EditorCore`
- **Breaking:** `EditorStyle::from_theme` needs the `FromTheme` trait in scope, and `ImageRejection::message` needs `ImageRejectionMessage`

### Fixed
- A double click sometimes selected only part of a word, when the mouse moved slightly during the click; dragging after a double or triple click now extends the selection by whole words or paragraphs

[unreleased]: https://github.com/iocast/frust-markdown-editor/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/iocast/frust-markdown-editor/releases/tag/v0.3.1
