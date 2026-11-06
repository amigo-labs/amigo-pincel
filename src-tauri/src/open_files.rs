//! Files the OS hands the app (launch argument, second instance, macOS
//! open events) reach the renderer as `open-file` events. An event emitted
//! before the page listens is lost, so paths that arrive before the
//! renderer asks for them wait here; `take_open_files` hands them over and
//! from then on each path is emitted directly (docs/specs/desktop-app.md
//! AC-04).

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// Paths waiting for the renderer, and whether it listens yet.
#[derive(Debug, Default)]
pub struct Pending {
    ready: bool,
    paths: Vec<String>,
}

impl Pending {
    /// Queues `path` while the renderer does not listen; returns it back
    /// when it should be emitted right away.
    fn offer(&mut self, path: String) -> Option<String> {
        if self.ready {
            Some(path)
        } else {
            self.paths.push(path);
            None
        }
    }

    /// The renderer listens from now on: hands over the queued paths.
    fn take(&mut self) -> Vec<String> {
        self.ready = true;
        std::mem::take(&mut self.paths)
    }

    /// The page (re)loads: its listener is gone until it asks again.
    fn reset(&mut self) {
        self.ready = false;
    }
}

/// Managed state behind [`deliver`] and [`take_open_files`].
#[derive(Debug, Default)]
pub struct OpenFiles(Mutex<Pending>);

/// Sends `path` to the renderer, or keeps it until the renderer asks.
pub fn deliver<R: Runtime>(app: &AppHandle<R>, path: String) {
    let state = app.state::<OpenFiles>();
    let now = state.0.lock().ok().and_then(|mut p| p.offer(path));
    if let Some(path) = now {
        let _ = app.emit("open-file", path);
    }
}

/// Marks the renderer as not listening (a page load started).
pub fn reset<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(mut p) = app.state::<OpenFiles>().0.lock() {
        p.reset();
    }
}

/// Called by the renderer once its `open-file` listener is armed; returns
/// the paths that arrived before.
#[tauri::command]
pub fn take_open_files(state: State<'_, OpenFiles>) -> Vec<String> {
    state.0.lock().map(|mut p| p.take()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_before_take_queues_path() {
        let mut p = Pending::default();
        assert_eq!(p.offer("/a.png".into()), None);
        assert_eq!(p.offer("/b.png".into()), None);
        assert_eq!(p.take(), vec!["/a.png".to_string(), "/b.png".to_string()]);
    }

    #[test]
    fn offer_after_take_emits_directly() {
        let mut p = Pending::default();
        assert!(p.take().is_empty());
        assert_eq!(p.offer("/a.png".into()), Some("/a.png".to_string()));
        assert!(p.take().is_empty());
    }

    #[test]
    fn reset_queues_again_until_next_take() {
        let mut p = Pending::default();
        p.take();
        p.reset();
        assert_eq!(p.offer("/a.png".into()), None);
        assert_eq!(p.take(), vec!["/a.png".to_string()]);
    }
}
