//! Tauri wiring for window events: closing the main window hides it.

use tauri::{Manager, Runtime, Window, WindowEvent};

use crate::shell::{self, CloseResponse};

/// Window-event hook for the app builder: turns a close of the main window into a hide.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && shell::on_close_requested(window.label(), window.app_handle()) == CloseResponse::Prevent
    {
        api.prevent_close();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tauri::test::mock_app;
    use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

    use super::*;
    use crate::shell::MAIN_WINDOW;

    /// What happened after we asked the main window to close.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Outcome {
        /// The event loop carried on with `main` still registered.
        Survived,
        /// The close went through and the app began exiting.
        Closed,
    }

    // Drives the mock event loop (which sleeps 1s per iteration, so this takes
    // about a second): on Ready, request a close of `main`; once the
    // CloseRequested event has been handled, note whether the window survived,
    // then destroy it so the loop exits.
    //
    // Limits of the tauri 2.12.1 mock runtime shape this test:
    // - It never calls builder-level `on_window_event` listeners (its
    //   dispatcher drops them), only the `App::run` callback. So the test
    //   feeds the CloseRequested run event to `on_window_event` itself; the
    //   one-line builder wiring in `lib.rs` is covered by the manual demo.
    // - Its `is_visible()` is always true, so the hide can't be observed here;
    //   `shell::on_close_requested` tests cover it.
    #[test]
    fn close_request_on_main_keeps_the_window() {
        let app = mock_app();
        WebviewWindowBuilder::new(&app, MAIN_WINDOW, WebviewUrl::default())
            .build()
            .expect("main window builds");

        let outcome = Arc::new(Mutex::new(None));
        let recorded = Arc::clone(&outcome);
        let mut close_seen = false;
        app.run(move |handle, event| {
            let mut recorded = recorded.lock().unwrap();
            match event {
                RunEvent::Ready => handle
                    .get_webview_window(MAIN_WINDOW)
                    .expect("main window exists")
                    .close()
                    .expect("close request is sent"),
                RunEvent::WindowEvent {
                    label,
                    event: event @ WindowEvent::CloseRequested { .. },
                    ..
                } if label == MAIN_WINDOW => {
                    let window = handle
                        .get_webview_window(&label)
                        .expect("main window exists");
                    on_window_event(&window.as_ref().window(), &event);
                    close_seen = true;
                }
                RunEvent::MainEventsCleared if close_seen && recorded.is_none() => {
                    let window = handle.get_webview_window(MAIN_WINDOW);
                    *recorded = Some(if window.is_some() {
                        Outcome::Survived
                    } else {
                        Outcome::Closed
                    });
                    if let Some(window) = window {
                        window.destroy().expect("destroy is sent");
                    }
                }
                RunEvent::ExitRequested { .. } if recorded.is_none() => {
                    *recorded = Some(Outcome::Closed);
                }
                _ => {}
            }
        });

        assert_eq!(*outcome.lock().unwrap(), Some(Outcome::Survived));
    }
}
