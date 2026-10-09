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

/// Label of the app's single window, as configured in `tauri.conf.json`.
pub const MAIN_WINDOW: &str = "main";

/// What the shell needs from the OS. The real impl wraps `AppHandle`; tests use a fake.
pub trait Shell {
    /// Whether the main window is currently shown. A missing window counts as hidden.
    fn window_visible(&self) -> bool;
    /// Shows and focuses the main window.
    fn show_window(&self);
    /// Hides the main window without destroying it.
    fn hide_window(&self);
    fn quit(&self);
}

/// Performs `action` against `shell`.
pub fn dispatch(action: MenuAction, shell: &impl Shell) {
    match action {
        MenuAction::ToggleWindow if shell.window_visible() => shell.hide_window(),
        MenuAction::ToggleWindow => shell.show_window(),
        MenuAction::Quit => shell.quit(),
    }
}

/// What to do with a window's close request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResponse {
    /// Let the window close.
    Allow,
    /// Keep the window alive (it has been hidden instead).
    Prevent,
}

/// Handles a close request for the window labelled `label`.
///
/// Closing the main window hides it instead, so the tray's Show/Hide can bring
/// the same window back. Other windows close normally.
pub fn on_close_requested(label: &str, shell: &impl Shell) -> CloseResponse {
    if label == MAIN_WINDOW {
        shell.hide_window();
        CloseResponse::Prevent
    } else {
        CloseResponse::Allow
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;

    use super::*;

    /// A state-changing call made to the fake. Queries are not recorded.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Call {
        Show,
        Hide,
        Quit,
    }

    /// Records every state-changing call and tracks window visibility.
    #[derive(Default)]
    struct FakeShell {
        visible: Cell<bool>,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeShell {
        fn with_window_visible(visible: bool) -> Self {
            Self {
                visible: Cell::new(visible),
                ..Self::default()
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl Shell for FakeShell {
        fn window_visible(&self) -> bool {
            self.visible.get()
        }

        fn show_window(&self) {
            self.visible.set(true);
            self.calls.borrow_mut().push(Call::Show);
        }

        fn hide_window(&self) {
            self.visible.set(false);
            self.calls.borrow_mut().push(Call::Hide);
        }

        fn quit(&self) {
            self.calls.borrow_mut().push(Call::Quit);
        }
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

    #[test]
    fn toggle_hides_a_visible_window_and_does_nothing_else() {
        let shell = FakeShell::with_window_visible(true);
        dispatch(MenuAction::ToggleWindow, &shell);
        assert_eq!(shell.calls(), [Call::Hide]);
    }

    #[test]
    fn toggle_shows_a_hidden_window_and_does_nothing_else() {
        let shell = FakeShell::with_window_visible(false);
        dispatch(MenuAction::ToggleWindow, &shell);
        assert_eq!(shell.calls(), [Call::Show]);
    }

    #[test]
    fn two_toggles_return_to_the_starting_state() {
        for start in [true, false] {
            let shell = FakeShell::with_window_visible(start);
            dispatch(MenuAction::ToggleWindow, &shell);
            assert_eq!(shell.window_visible(), !start);
            dispatch(MenuAction::ToggleWindow, &shell);
            assert_eq!(shell.window_visible(), start);
        }
    }

    #[test]
    fn closing_the_main_window_hides_it_and_prevents_the_close() {
        let shell = FakeShell::with_window_visible(true);
        assert_eq!(
            on_close_requested(MAIN_WINDOW, &shell),
            CloseResponse::Prevent
        );
        assert_eq!(shell.calls(), [Call::Hide]);
    }

    #[test]
    fn closing_another_window_is_allowed_and_touches_nothing() {
        let shell = FakeShell::with_window_visible(true);
        assert_eq!(on_close_requested("other", &shell), CloseResponse::Allow);
        assert_eq!(shell.calls(), []);
    }
}
