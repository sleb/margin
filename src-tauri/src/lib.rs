mod shell;
mod tray;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::init(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
