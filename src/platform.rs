//! Modifier keys held right now, asked from the operating system.
//!
//! Frust's pointer events carry no modifier state and bare modifier presses
//! produce no key events, so Shift+click (extend the selection), Ctrl/Cmd +
//! click (follow a link) and Shift+wheel (scroll sideways) ask the OS
//! directly. Supported on Windows and macOS; elsewhere `None`.

use crate::keyboard::Modifiers;

/// The modifiers held at this moment, if the platform can tell.
pub(crate) fn modifiers() -> Option<Modifiers> {
    imp::modifiers()
}

#[cfg(target_os = "windows")]
mod imp {
    use crate::keyboard::Modifiers;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetKeyState(virtual_key: i32) -> i16;
    }

    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;

    fn down(key: i32) -> bool {
        // SAFETY: `GetKeyState` only reads the calling thread's keyboard
        // state; any virtual-key code is a valid argument.
        unsafe { GetKeyState(key) < 0 }
    }

    pub(super) fn modifiers() -> Option<Modifiers> {
        let mut modifiers = Modifiers::empty();
        for (key, flag) in [
            (VK_SHIFT, Modifiers::SHIFT),
            (VK_CONTROL, Modifiers::CTRL),
            (VK_MENU, Modifiers::ALT),
            (VK_LWIN, Modifiers::LOGO),
            (VK_RWIN, Modifiers::LOGO),
        ] {
            if down(key) {
                modifiers |= flag;
            }
        }
        Some(modifiers)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use crate::keyboard::Modifiers;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
    }

    const COMBINED_SESSION_STATE: i32 = 0;
    const SHIFT: u64 = 0x0002_0000;
    const CONTROL: u64 = 0x0004_0000;
    const ALTERNATE: u64 = 0x0008_0000;
    const COMMAND: u64 = 0x0010_0000;

    pub(super) fn modifiers() -> Option<Modifiers> {
        // SAFETY: a pure query of the session's current modifier flags.
        let flags = unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) };
        let mut modifiers = Modifiers::empty();
        for (mask, flag) in [
            (SHIFT, Modifiers::SHIFT),
            (CONTROL, Modifiers::CTRL),
            (ALTERNATE, Modifiers::ALT),
            (COMMAND, Modifiers::LOGO),
        ] {
            if flags & mask != 0 {
                modifiers |= flag;
            }
        }
        Some(modifiers)
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    use crate::keyboard::Modifiers;

    pub(super) fn modifiers() -> Option<Modifiers> {
        None
    }
}
