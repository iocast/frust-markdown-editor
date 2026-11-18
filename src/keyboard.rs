//! Keys and modifiers, as used by [`KeyBinding`](crate::KeyBinding).
//!
//! Frust delivers key presses as `frust::authoring::KeyEvent`; the widget
//! converts them to these types.

use std::ops::{BitOr, BitOrAssign};

/// A key that is not a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Named {
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Escape,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    /// Not delivered by Frust 0.6 desktop and web shells.
    PageUp,
    /// Not delivered by Frust 0.6 desktop and web shells.
    PageDown,
    Copy,
    Cut,
    Paste,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// A logical key: named, or the character(s) it produces.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key<C = String> {
    Named(Named),
    Character(C),
    Unidentified,
}

impl Key {
    /// A borrowed view, convenient for `match`.
    pub fn as_ref(&self) -> Key<&str> {
        match self {
            Key::Named(named) => Key::Named(*named),
            Key::Character(c) => Key::Character(c.as_str()),
            Key::Unidentified => Key::Unidentified,
        }
    }
}

/// Modifier keys held during a key press or click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const SHIFT: Modifiers = Modifiers(1);
    pub const CTRL: Modifiers = Modifiers(2);
    pub const ALT: Modifiers = Modifiers(4);
    /// Windows key / Command key / Super key.
    pub const LOGO: Modifiers = Modifiers(8);
    /// `Ctrl` on Windows and Linux, `Cmd` on macOS.
    pub const COMMAND: Modifiers = if cfg!(target_os = "macos") {
        Modifiers::LOGO
    } else {
        Modifiers::CTRL
    };

    pub const fn empty() -> Modifiers {
        Modifiers(0)
    }

    pub const fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn shift(self) -> bool {
        self.contains(Modifiers::SHIFT)
    }

    pub fn control(self) -> bool {
        self.contains(Modifiers::CTRL)
    }

    pub fn alt(self) -> bool {
        self.contains(Modifiers::ALT)
    }

    pub fn logo(self) -> bool {
        self.contains(Modifiers::LOGO)
    }

    /// `Ctrl` on Windows and Linux, `Cmd` on macOS.
    pub fn command(self) -> bool {
        self.contains(Modifiers::COMMAND)
    }

    pub(crate) fn from_frust(modifiers: frust::authoring::Modifiers) -> Modifiers {
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
}

impl BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, other: Modifiers) {
        self.0 |= other.0;
    }
}

/// A wheel movement: lines (mouse wheel) or pixels (touchpad).
/// Positive `y` scrolls up / zooms in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ScrollDelta {
    Lines { x: f32, y: f32 },
    Pixels { x: f32, y: f32 },
}

/// A Frust key press as `(key, modifiers, text)`. `text` is what the key
/// types, `None` while Ctrl / Cmd is held (then it is a shortcut).
pub(crate) fn from_frust(
    event: &frust::authoring::KeyEvent,
) -> Option<(Key, Modifiers, Option<String>)> {
    use frust::authoring::{Key as FrustKey, NamedKey};

    let modifiers = Modifiers::from_frust(event.modifiers);
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
    fn modifier_flags() {
        let both = Modifiers::COMMAND | Modifiers::SHIFT;
        assert!(both.command());
        assert!(both.shift());
        assert!(!both.alt());
        assert!(Modifiers::empty().is_empty());
    }

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
