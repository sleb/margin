mod shell;
mod tray;
mod window;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::init(app.handle())?;
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
