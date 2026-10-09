//! Tauri wiring for the menubar tray icon.
//!
//! Builds the tray menu from [`shell::MENU`], routes menu clicks through
//! [`shell::dispatch`], and implements [`Shell`] for the real `AppHandle`.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use crate::shell::{self, MAIN_WINDOW, MenuAction, MenuEntry, Shell};

/// Id of the single tray icon.
const TRAY_ID: &str = "main";

/// The tray icon: a template image (black on transparent) so macOS tints it for light/dark mode.
pub(crate) const TRAY_PNG: &[u8] = include_bytes!("../icons/tray.png");

/// Creates the menubar tray icon and its menu.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_PNG)?)
        .icon_as_template(true)
        .tooltip("margin")
        .menu(&build_menu(app)?)
        .on_menu_event(|app, event| {
            if let Some(action) = MenuAction::from_id(event.id().as_ref()) {
                shell::dispatch(action, app);
            }
        })
        .build(app)?;
    Ok(())
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for entry in shell::MENU {
        match *entry {
            MenuEntry::Item { action, label } => {
                menu.append(&MenuItem::with_id(
                    app,
                    action.id(),
                    label,
                    true,
                    None::<&str>,
                )?)?;
            }
            MenuEntry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

impl<R: Runtime> Shell for AppHandle<R> {
    fn window_visible(&self) -> bool {
        main_window(self).is_some_and(|window| {
            window
                .is_visible()
                .inspect_err(|err| eprintln!("margin: can't read main window visibility: {err}"))
                .unwrap_or(false)
        })
    }

    fn show_window(&self) {
        if let Some(window) = main_window(self)
            && let Err(err) = window.show().and_then(|()| window.set_focus())
        {
            eprintln!("margin: can't show main window: {err}");
        }
    }

    fn hide_window(&self) {
        if let Some(window) = main_window(self)
            && let Err(err) = window.hide()
        {
            eprintln!("margin: can't hide main window: {err}");
        }
    }

    fn quit(&self) {
        self.exit(0);
    }
}

/// Looks up the main window, logging when it's missing so callers can no-op.
fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let window = app.get_webview_window(MAIN_WINDOW);
    if window.is_none() {
        eprintln!("margin: no `{MAIN_WINDOW}` window; ignoring");
    }
    window
}

#[cfg(test)]
mod tests {
    use tauri::test::mock_app;
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    use super::*;

    #[test]
    fn shell_without_a_main_window_reports_hidden_and_ignores_show_hide() {
        let app = mock_app();
        let handle = app.handle();
        assert!(!handle.window_visible());
        handle.show_window();
        handle.hide_window();
        assert!(!handle.window_visible());
    }

    // The tauri 2.12.1 mock runtime does not track visibility: `is_visible()`
    // always returns `Ok(true)` and `show()`/`hide()` are no-ops. So this can
    // only check that `show_window` finds the `main` window and then reports it
    // visible; "hide_window then not visible" is untestable on the mock.
    #[test]
    fn shell_shows_the_main_window_and_reports_it_visible() {
        let app = mock_app();
        WebviewWindowBuilder::new(&app, MAIN_WINDOW, WebviewUrl::default())
            .build()
            .expect("main window builds");
        let handle = app.handle();
        assert!(handle.get_webview_window(MAIN_WINDOW).is_some());

        handle.show_window();
        assert!(handle.window_visible());
    }

    #[test]
    fn tray_icon_is_a_square_template_image() {
        let image = Image::from_bytes(TRAY_PNG).expect("tray.png decodes");
        assert_eq!(image.width(), image.height(), "tray icon is square");

        let (pixels, rest) = image.rgba().as_chunks::<4>();
        assert!(rest.is_empty(), "RGBA buffer is whole pixels");
        assert!(
            pixels.iter().all(|px| px[..3] == [0, 0, 0]),
            "every pixel's RGB is black; alpha carries the shape"
        );
        assert!(
            pixels.iter().any(|px| px[3] > 0),
            "at least one pixel is non-transparent"
        );
    }
}
