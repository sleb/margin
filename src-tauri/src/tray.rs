//! Tauri wiring for the menubar tray icon.
//!
//! Builds the tray menu from [`shell::MENU`], routes menu clicks through
//! [`shell::dispatch`], and implements [`Shell`] for the real `AppHandle`.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};

use crate::shell::{self, MenuAction, MenuEntry, Shell};

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
    fn quit(&self) {
        self.exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
