mod shell;
mod tray;
mod window;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(activation_policy());
            tray::init(app.handle())?;
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// How margin presents itself to macOS: a menubar-only app, absent from the Dock and Cmd-Tab.
#[cfg(target_os = "macos")]
fn activation_policy() -> tauri::ActivationPolicy {
    tauri::ActivationPolicy::Accessory
}

#[cfg(test)]
mod tests {
    use crate::shell::MAIN_WINDOW;

    #[test]
    fn main_window_is_hidden_at_launch() {
        let context: tauri::Context<tauri::test::MockRuntime> =
            tauri::generate_context!(test = true);
        let main = context
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == MAIN_WINDOW)
            .expect("tauri.conf.json declares the main window");
        assert!(!main.visible, "main window must start hidden");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn activation_policy_is_accessory() {
        assert!(matches!(
            super::activation_policy(),
            tauri::ActivationPolicy::Accessory
        ));
    }
}
