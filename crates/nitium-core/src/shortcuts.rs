//! Application-level keyboard shortcuts.
//!
//! Resolution is a pure function of the key text and modifiers so that it can be
//! tested without a window or UI toolkit. Shortcuts are matched before the focused
//! element sees the key; anything that does not resolve falls through to the page.

/// Special key codepoints matching Slint's `slint::platform::Key` encoding.
pub mod keys {
    pub const TAB: &str = "\u{0009}";
    pub const BACKTAB: &str = "\u{0019}";
    pub const ESCAPE: &str = "\u{001b}";
    pub const LEFT_ARROW: &str = "\u{f702}";
    pub const RIGHT_ARROW: &str = "\u{f703}";
    pub const F3: &str = "\u{f706}";
    pub const F5: &str = "\u{f708}";
    pub const F6: &str = "\u{f709}";
    pub const PAGE_UP: &str = "\u{f72c}";
    pub const PAGE_DOWN: &str = "\u{f72d}";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewTab,
    CloseTab,
    ReopenClosedTab,
    FocusAddress,
    ToggleBookmark,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// Open the find bar (Ctrl+F).
    Find,
    FindNext,
    FindPrevious,
    Reload,
    Back,
    Forward,
    NextTab,
    PreviousTab,
    /// Zero-based index of the tab to select (Ctrl+1 .. Ctrl+8).
    SelectTab(usize),
    SelectLastTab,
    /// Open settings overlay (Ctrl+,).
    Settings,
    /// Open bookmarks overlay (Ctrl+Shift+O).
    Bookmarks,
    /// Open history overlay (Ctrl+H).
    History,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    /// The platform's "command" modifier: Cmd on macOS, Ctrl elsewhere.
    fn command(self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.control
        }
    }
}

/// Maps a key press to an application shortcut, if it is one.
pub fn resolve(text: &str, mods: Modifiers) -> Option<Shortcut> {
    let command = mods.command();

    // Keys without a command modifier.
    if !command {
        if mods.alt && !mods.shift {
            if text == keys::LEFT_ARROW {
                return Some(Shortcut::Back);
            }
            if text == keys::RIGHT_ARROW {
                return Some(Shortcut::Forward);
            }
            if text.eq_ignore_ascii_case("d") {
                return Some(Shortcut::FocusAddress);
            }
        }
        if !mods.alt && text == keys::F3 {
            return Some(if mods.shift {
                Shortcut::FindPrevious
            } else {
                Shortcut::FindNext
            });
        }
        if !mods.alt && !mods.shift {
            if text == keys::F5 {
                return Some(Shortcut::Reload);
            }
            if text == keys::F6 {
                return Some(Shortcut::FocusAddress);
            }
        }
        return None;
    }
    if mods.alt {
        return None;
    }

    // Tab switching.
    if text == keys::BACKTAB || (text == keys::TAB && mods.shift) {
        return Some(Shortcut::PreviousTab);
    }
    if text == keys::TAB {
        return Some(Shortcut::NextTab);
    }
    if text == keys::PAGE_UP {
        return Some(Shortcut::PreviousTab);
    }
    if text == keys::PAGE_DOWN {
        return Some(Shortcut::NextTab);
    }

    let mut chars = text.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return None;
    };
    match (ch.to_ascii_lowercase(), mods.shift) {
        ('t', false) => Some(Shortcut::NewTab),
        ('t', true) => Some(Shortcut::ReopenClosedTab),
        ('w', false) => Some(Shortcut::CloseTab),
        ('l', false) => Some(Shortcut::FocusAddress),
        ('d', false) => Some(Shortcut::ToggleBookmark),
        ('f', false) => Some(Shortcut::Find),
        ('g', false) => Some(Shortcut::FindNext),
        ('g', true) => Some(Shortcut::FindPrevious),
        // `+` needs Shift on most layouts, so `=` (the same key) zooms in as well.
        ('+', _) | ('=', _) => Some(Shortcut::ZoomIn),
        ('-', _) => Some(Shortcut::ZoomOut),
        ('0', false) => Some(Shortcut::ZoomReset),
        ('r', false) => Some(Shortcut::Reload),
        ('9', false) => Some(Shortcut::SelectLastTab),
        (digit @ '1'..='8', false) => Some(Shortcut::SelectTab(digit as usize - '1' as usize)),
        (',', false) => Some(Shortcut::Settings),
        ('o', true) => Some(Shortcut::Bookmarks),
        ('h', false) => Some(Shortcut::History),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers {
        control: true,
        shift: false,
        alt: false,
        meta: false,
    };

    fn ctrl_shift() -> Modifiers {
        Modifiers {
            shift: true,
            ..CTRL
        }
    }

    // The "command" modifier is Cmd on macOS, so these tests describe non-mac behaviour.
    #[cfg(not(target_os = "macos"))]
    mod command_shortcuts {
        use super::*;

        #[test]
        fn tab_management() {
            assert_eq!(resolve("t", CTRL), Some(Shortcut::NewTab));
            assert_eq!(resolve("w", CTRL), Some(Shortcut::CloseTab));
            assert_eq!(resolve("T", ctrl_shift()), Some(Shortcut::ReopenClosedTab));
            assert_eq!(resolve("t", ctrl_shift()), Some(Shortcut::ReopenClosedTab));
        }

        #[test]
        fn tab_switching() {
            assert_eq!(resolve(keys::TAB, CTRL), Some(Shortcut::NextTab));
            assert_eq!(resolve(keys::TAB, ctrl_shift()), Some(Shortcut::PreviousTab));
            assert_eq!(
                resolve(keys::BACKTAB, ctrl_shift()),
                Some(Shortcut::PreviousTab)
            );
            assert_eq!(resolve("1", CTRL), Some(Shortcut::SelectTab(0)));
            assert_eq!(resolve("8", CTRL), Some(Shortcut::SelectTab(7)));
            assert_eq!(resolve("9", CTRL), Some(Shortcut::SelectLastTab));
        }

        #[test]
        fn address_and_reload() {
            assert_eq!(resolve("l", CTRL), Some(Shortcut::FocusAddress));
            assert_eq!(resolve("r", CTRL), Some(Shortcut::Reload));
        }

        #[test]
        fn settings_bookmarks_history() {
            assert_eq!(resolve(",", CTRL), Some(Shortcut::Settings));
            assert_eq!(resolve("o", ctrl_shift()), Some(Shortcut::Bookmarks));
            assert_eq!(resolve("O", ctrl_shift()), Some(Shortcut::Bookmarks));
            assert_eq!(resolve("h", CTRL), Some(Shortcut::History));
            assert_eq!(resolve("H", CTRL), Some(Shortcut::History));
        }

        #[test]
        fn zoom() {
            assert_eq!(resolve("=", CTRL), Some(Shortcut::ZoomIn));
            assert_eq!(resolve("+", ctrl_shift()), Some(Shortcut::ZoomIn));
            assert_eq!(resolve("-", CTRL), Some(Shortcut::ZoomOut));
            assert_eq!(resolve("0", CTRL), Some(Shortcut::ZoomReset));
        }

        #[test]
        fn find() {
            assert_eq!(resolve("f", CTRL), Some(Shortcut::Find));
            assert_eq!(resolve("g", CTRL), Some(Shortcut::FindNext));
            assert_eq!(resolve("G", ctrl_shift()), Some(Shortcut::FindPrevious));
        }

        #[test]
        fn bookmark() {
            assert_eq!(resolve("d", CTRL), Some(Shortcut::ToggleBookmark));
            assert_eq!(resolve("D", CTRL), Some(Shortcut::ToggleBookmark));
        }

        #[test]
        fn leaves_page_shortcuts_alone() {
            // Copy/paste/select-all/undo belong to the page.
            for key in ["c", "v", "x", "a", "z"] {
                assert_eq!(resolve(key, CTRL), None, "ctrl+{key}");
            }
        }

        #[test]
        fn plain_typing_is_not_a_shortcut() {
            assert_eq!(resolve("t", Modifiers::default()), None);
            assert_eq!(resolve("w", Modifiers::default()), None);
        }
    }

    #[test]
    fn navigation_keys() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(resolve(keys::LEFT_ARROW, alt), Some(Shortcut::Back));
        assert_eq!(resolve(keys::RIGHT_ARROW, alt), Some(Shortcut::Forward));
        assert_eq!(resolve("d", alt), Some(Shortcut::FocusAddress));
        assert_eq!(
            resolve(keys::F5, Modifiers::default()),
            Some(Shortcut::Reload)
        );
        assert_eq!(
            resolve(keys::F6, Modifiers::default()),
            Some(Shortcut::FocusAddress)
        );
    }

    #[test]
    fn f3_steps_through_find_results() {
        assert_eq!(
            resolve(keys::F3, Modifiers::default()),
            Some(Shortcut::FindNext)
        );
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(resolve(keys::F3, shift), Some(Shortcut::FindPrevious));
    }

    #[test]
    fn plain_arrows_go_to_the_page() {
        assert_eq!(resolve(keys::LEFT_ARROW, Modifiers::default()), None);
    }
}
