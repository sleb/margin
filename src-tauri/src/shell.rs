//! Pure, Tauri-free shell logic: the menu spec and what each menu action does.
//!
//! This module is the unit-test seam for the menubar shell. `tray.rs` turns
//! [`MENU`] into a real Tauri menu and implements [`Shell`] for `AppHandle`.

/// An action the user can trigger from the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuAction {
    ToggleWindow,
    Quit,
}

impl MenuAction {
    /// Every action, for exhaustive iteration in tests and lookups.
    pub const ALL: [Self; 2] = [Self::ToggleWindow, Self::Quit];

    /// The stable menu-item id for this action.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToggleWindow => "toggle-window",
            Self::Quit => "quit",
        }
    }

    /// Maps a menu-item id back to its action.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

/// One entry in the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        action: MenuAction,
        label: &'static str,
    },
    Separator,
}

/// The tray menu, top to bottom.
pub const MENU: &[MenuEntry] = &[
    MenuEntry::Item {
        action: MenuAction::ToggleWindow,
        label: "Show/Hide",
    },
    MenuEntry::Separator,
    MenuEntry::Item {
        action: MenuAction::Quit,
        label: "Quit",
    },
];

/// What the shell needs from the OS. The real impl wraps `AppHandle`; tests use a fake.
///
/// Window visibility methods arrive with Show/Hide in slice 2 of #3.
pub trait Shell {
    fn quit(&self);
}

/// Performs `action` against `shell`.
pub fn dispatch(action: MenuAction, shell: &impl Shell) {
    match action {
        // Show/Hide behavior lands in slice 2 of #3; until then the item is inert.
        MenuAction::ToggleWindow => {}
        MenuAction::Quit => shell.quit(),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Call {
        Quit,
    }

    /// Records every call made to it.
    #[derive(Default)]
    struct FakeShell {
        calls: RefCell<Vec<Call>>,
    }

    impl FakeShell {
        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl Shell for FakeShell {
        fn quit(&self) {
            self.calls.borrow_mut().push(Call::Quit);
        }
    }

    #[test]
    fn menu_is_show_hide_separator_quit() {
        assert_eq!(
            MENU,
            &[
                MenuEntry::Item {
                    action: MenuAction::ToggleWindow,
                    label: "Show/Hide",
                },
                MenuEntry::Separator,
                MenuEntry::Item {
                    action: MenuAction::Quit,
                    label: "Quit",
                },
            ]
        );
    }

    #[test]
    fn from_id_round_trips_every_action() {
        for action in MenuAction::ALL {
            assert_eq!(MenuAction::from_id(action.id()), Some(action));
        }
    }

    #[test]
    fn from_id_rejects_unknown_id() {
        assert_eq!(MenuAction::from_id("nope"), None);
    }

    #[test]
    fn action_ids_are_unique_and_non_empty() {
        let ids: HashSet<_> = MenuAction::ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids.len(), MenuAction::ALL.len());
        assert!(ids.iter().all(|id| !id.is_empty()));
    }

    #[test]
    fn dispatch_quit_calls_quit_once_and_nothing_else() {
        let shell = FakeShell::default();
        dispatch(MenuAction::Quit, &shell);
        assert_eq!(shell.calls(), [Call::Quit]);
    }
}
