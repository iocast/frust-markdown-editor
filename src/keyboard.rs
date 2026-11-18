//! Keys and modifiers, as used by [`KeyBinding`](crate::KeyBinding).
//!
//! Frust delivers key presses as `frust::authoring::KeyEvent`; the widget
//! converts them to the core's types.

pub use markdown_editor_core::keyboard::{Key, Modifiers, Named, ScrollDelta};

/// Frust's modifier state.
pub(crate) fn modifiers_from_frust(modifiers: frust::authoring::Modifiers) -> Modifiers {
    let mut result = Modifiers::empty();
    for (held, flag) in [
        (modifiers.shift, Modifiers::SHIFT),
        (modifiers.ctrl, Modifiers::CTRL),
        (modifiers.alt, Modifiers::ALT),
        (modifiers.meta, Modifiers::LOGO),
    ] {
        if held {
            result |= flag;
        }
    }
    result
}

/// A Frust key press as `(key, modifiers, text)`. `text` is what the key
/// types, `None` while Ctrl / Cmd is held (then it is a shortcut).
pub(crate) fn from_frust(
    event: &frust::authoring::KeyEvent,
) -> Option<(Key, Modifiers, Option<String>)> {
    use frust::authoring::{Key as FrustKey, NamedKey};

    let modifiers = modifiers_from_frust(event.modifiers);
    let named = |named| Some((Key::Named(named), modifiers, None));
    match &event.key {
        FrustKey::Character(text) if text == " " => {
            Some((Key::Named(Named::Space), modifiers, Some(text.clone())))
        }
        FrustKey::Character(text) => {
            let shortcut = (modifiers.control() || modifiers.logo()) && !modifiers.alt();
            let typed = (!shortcut).then(|| text.clone());
            Some((Key::Character(text.clone()), modifiers, typed))
        }
        FrustKey::Named(key) => match key {
            NamedKey::Enter => named(Named::Enter),
            NamedKey::Tab => named(Named::Tab),
            NamedKey::Backspace => named(Named::Backspace),
            NamedKey::Delete => named(Named::Delete),
            NamedKey::Insert => named(Named::Insert),
            NamedKey::Escape => named(Named::Escape),
            NamedKey::ArrowLeft => named(Named::ArrowLeft),
            NamedKey::ArrowRight => named(Named::ArrowRight),
            NamedKey::ArrowUp => named(Named::ArrowUp),
            NamedKey::ArrowDown => named(Named::ArrowDown),
            NamedKey::Home => named(Named::Home),
            NamedKey::End => named(Named::End),
            NamedKey::Copy => named(Named::Copy),
            NamedKey::Cut => named(Named::Cut),
            NamedKey::Paste => named(Named::Paste),
            NamedKey::F1 => named(Named::F1),
            NamedKey::F2 => named(Named::F2),
            NamedKey::F3 => named(Named::F3),
            NamedKey::F4 => named(Named::F4),
            NamedKey::F5 => named(Named::F5),
            NamedKey::F6 => named(Named::F6),
            NamedKey::F7 => named(Named::F7),
            NamedKey::F8 => named(Named::F8),
            NamedKey::F9 => named(Named::F9),
            NamedKey::F10 => named(Named::F10),
            NamedKey::F11 => named(Named::F11),
            NamedKey::F12 => named(Named::F12),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_frust_key_events() {
        use frust::authoring::{Key as FrustKey, KeyEvent, Modifiers as FrustModifiers};

        let press = |key: FrustKey, ctrl: bool| KeyEvent {
            key,
            modifiers: FrustModifiers {
                ctrl,
                ..FrustModifiers::default()
            },
            repeat: false,
        };
        let (key, _, text) = from_frust(&press(FrustKey::Character("a".into()), false)).unwrap();
        assert_eq!(key, Key::Character("a".into()));
        assert_eq!(text.as_deref(), Some("a"));

        let (_, modifiers, text) =
            from_frust(&press(FrustKey::Character("b".into()), true)).unwrap();
        assert!(modifiers.control());
        assert_eq!(text, None);

        let (key, _, text) = from_frust(&press(FrustKey::Character(" ".into()), false)).unwrap();
        assert_eq!(key, Key::Named(Named::Space));
        assert_eq!(text.as_deref(), Some(" "));
    }
}
