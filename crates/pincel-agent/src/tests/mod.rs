//! Tool-level tests over the public `call` entry point, with access to the
//! session internals for exact-state assertions.

mod doc;
mod draw;
mod image;
mod image_draw;
mod preview;
mod structure;

use std::collections::HashMap;

use pincel_core::{CelData, CelMap, FrameIndex, LayerId, Sprite};
use serde_json::Value;

use crate::image::ImageDoc;
use crate::workspace::Doc;
use crate::{AgentError, FileHost, ToolOutput, Workspace, call};

/// In-memory files keyed by workspace-relative path.
#[derive(Default)]
pub(crate) struct MemHost {
    pub files: HashMap<String, Vec<u8>>,
}

impl FileHost for MemHost {
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("{path}: no such file"))
    }

    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.files.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn check_write(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
}

/// One test session: a workspace and its file host.
#[derive(Default)]
pub(crate) struct S {
    pub ws: Workspace,
    pub host: MemHost,
}

impl S {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn call(&mut self, name: &str, args: Value) -> Result<ToolOutput, AgentError> {
        call(&mut self.ws, &mut self.host, name, args)
    }

    #[track_caller]
    pub fn ok(&mut self, name: &str, args: Value) -> Value {
        match self.call(name, args) {
            Ok(out) => out.json,
            Err(e) => panic!("{name} failed: {e}"),
        }
    }

    #[track_caller]
    pub fn err(&mut self, name: &str, args: Value) -> String {
        match self.call(name, args) {
            Ok(out) => panic!("{name} unexpectedly succeeded: {}", out.json),
            Err(e) => e.to_string(),
        }
    }

    /// A new `w`×`h` Pixel document; returns its id.
    pub fn new_doc(&mut self, w: u32, h: u32) -> String {
        let out = self.ok(
            "doc.new",
            serde_json::json!({ "mode": "pixel", "width": w, "height": h }),
        );
        out["doc"].as_str().expect("doc id").to_owned()
    }

    pub fn doc(&self, id: &str) -> &Doc {
        self.ws.get(id).expect("open document")
    }

    /// A new `w`×`h` Image document; returns its id.
    pub fn new_image(&mut self, w: u32, h: u32) -> String {
        let out = self.ok(
            "doc.new",
            serde_json::json!({ "mode": "image", "width": w, "height": h }),
        );
        out["doc"].as_str().expect("doc id").to_owned()
    }

    pub fn image(&self, id: &str) -> &ImageDoc {
        self.ws.get_image(id).expect("open Image document")
    }

    /// Exact document state.
    pub fn state(&self, id: &str) -> (Sprite, CelMap) {
        let d = self.doc(id);
        (d.sprite.clone(), d.cels.clone())
    }

    /// Canvas-sized RGBA of the cel at `(layer, frame)` (transparent
    /// outside the cel or without one).
    pub fn cel_rgba(&self, id: &str, layer: u32, frame: u32) -> Vec<u8> {
        let d = self.doc(id);
        let (w, h) = (d.sprite.width as i64, d.sprite.height as i64);
        let mut out = vec![0u8; (w * h * 4) as usize];
        let Some(cel) = d.cels.get(LayerId::new(layer), FrameIndex::new(frame)) else {
            return out;
        };
        let CelData::Image(buf) = &cel.data else {
            return out;
        };
        for y in 0..buf.height as i64 {
            for x in 0..buf.width as i64 {
                let (sx, sy) = (x + cel.position.0 as i64, y + cel.position.1 as i64);
                if sx < 0 || sy < 0 || sx >= w || sy >= h {
                    continue;
                }
                let src = ((y * buf.width as i64 + x) * 4) as usize;
                let dst = ((sy * w + sx) * 4) as usize;
                out[dst..dst + 4].copy_from_slice(&buf.data[src..src + 4]);
            }
        }
        out
    }
}

/// RGBA of pixel `(x, y)` in a canvas-sized buffer of width `w`.
pub(crate) fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}
