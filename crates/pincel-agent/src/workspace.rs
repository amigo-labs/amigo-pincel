//! The in-memory session: open documents, the active one, and the
//! per-document undo bus.

use pincel_core::{
    AnyCommand, Bus, Cel, CelMap, ColorMode, CommandBatch, Frame, FrameIndex, Layer, LayerId,
    LayerKind, Palette, PixelBuffer, Sprite,
};
use serde_json::{Value, json};

use crate::AgentError;
use crate::args::Args;

/// One open Pixel document.
#[derive(Debug)]
pub(crate) struct Doc {
    pub sprite: Sprite,
    pub cels: CelMap,
    pub bus: Bus,
    /// The layer the agent selected; resolved by the editor's paint-target
    /// rule ([`Sprite::paint_target_layer`]).
    pub active_layer: Option<LayerId>,
    pub active_frame: FrameIndex,
    /// The file the document was opened from or last saved to.
    pub path: Option<String>,
    /// Changed since it was opened, created or last saved.
    pub dirty: bool,
}

impl Doc {
    /// A new document exactly like the editor's New Pixel document: one
    /// image layer `Layer 1` with an empty canvas-sized cel, one 100 ms
    /// frame and `palette` (crates/pincel-wasm `Document::new`).
    pub fn new_pixel(width: u32, height: u32, palette: Palette) -> Result<Self, AgentError> {
        let layer = LayerId::new(0);
        let sprite = Sprite::builder(width, height)
            .color_mode(ColorMode::Rgba)
            .add_layer(Layer::image(layer, "Layer 1"))
            .add_frame(Frame::new(100))
            .palette(palette)
            .build()
            .map_err(|e| AgentError::tool(format!("cannot create document: {e}")))?;
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            layer,
            FrameIndex::new(0),
            PixelBuffer::empty(width, height, ColorMode::Rgba),
        ));
        Ok(Self::from_parts(sprite, cels, None))
    }

    pub fn from_parts(sprite: Sprite, cels: CelMap, path: Option<String>) -> Self {
        Self {
            sprite,
            cels,
            bus: Bus::new(),
            active_layer: None,
            active_frame: FrameIndex::new(0),
            path,
            dirty: false,
        }
    }

    /// Run `cmds` as exactly one undo step (sealed on both sides, grouped
    /// in a [`CommandBatch`] when there are several). An empty list
    /// records nothing. A failing command leaves the document unchanged.
    pub fn commit(&mut self, mut cmds: Vec<AnyCommand>) -> Result<(), AgentError> {
        let cmd = match cmds.len() {
            0 => return Ok(()),
            1 => cmds.remove(0),
            _ => CommandBatch::new(cmds).into(),
        };
        self.bus.seal();
        let result = self.bus.execute(cmd, &mut self.sprite, &mut self.cels);
        self.bus.seal();
        result.map_err(|e| AgentError::tool(e.to_string()))?;
        self.dirty = true;
        self.clamp_active();
        Ok(())
    }

    /// Keep the active frame in range and drop an active layer that no
    /// longer exists (after a removal or an undo).
    pub fn clamp_active(&mut self) {
        let frames = self.sprite.frames.len() as u32;
        if self.active_frame.0 >= frames {
            self.active_frame = FrameIndex::new(frames.saturating_sub(1));
        }
        if let Some(id) = self.active_layer
            && self.sprite.layer(id).is_none()
        {
            self.active_layer = None;
        }
    }

    /// The layer and frame a pixel tool acts on: the explicit `layer` /
    /// `frame` arguments, else the active layer (editor paint-target rule)
    /// and the active frame. Refuses non-image and locked layers.
    pub fn paint_target(&self, args: &Args) -> Result<(LayerId, FrameIndex), AgentError> {
        let layer = match args.u32("layer")? {
            Some(id) => {
                let id = LayerId::new(id);
                let layer = self
                    .sprite
                    .layer(id)
                    .ok_or_else(|| AgentError::tool(format!("no layer with id {}", id.0)))?;
                if !matches!(layer.kind, LayerKind::Image) {
                    return Err(AgentError::tool(format!(
                        "layer {} ({:?}) is not an image layer; pixel tools need an image layer",
                        id.0, layer.name
                    )));
                }
                id
            }
            None => self
                .sprite
                .paint_target_layer(self.active_layer)
                .ok_or_else(|| AgentError::tool("document has no image layer to paint on"))?,
        };
        let frame = self.frame_arg(args)?.unwrap_or(self.active_frame);
        self.check_unlocked(layer)?;
        Ok((layer, frame))
    }

    /// The optional `frame` argument, checked against the frame count.
    pub fn frame_arg(&self, args: &Args) -> Result<Option<FrameIndex>, AgentError> {
        args.u32("frame")?.map(|f| self.check_frame(f)).transpose()
    }

    pub fn check_frame(&self, frame: u32) -> Result<FrameIndex, AgentError> {
        let count = self.sprite.frames.len();
        if (frame as usize) < count {
            Ok(FrameIndex::new(frame))
        } else {
            Err(AgentError::tool(format!(
                "frame {frame} does not exist (document has {count} frames, indices 0–{})",
                count.saturating_sub(1)
            )))
        }
    }

    /// Refuse to edit a locked layer or a layer inside a locked group.
    pub fn check_unlocked(&self, layer: LayerId) -> Result<(), AgentError> {
        if self.sprite.layer_locked(layer) {
            let name = self.sprite.layer(layer).map_or("", |l| l.name.as_str());
            return Err(AgentError::tool(format!(
                "layer {} ({name:?}) is locked (or inside a locked group); unlock it with layer.set {{\"locked\": false}}",
                layer.0
            )));
        }
        Ok(())
    }

    /// The cel at `(layer, frame)`, or an empty canvas-sized one.
    pub fn cel_or_empty(&self, layer: LayerId, frame: FrameIndex) -> Cel {
        self.cels.get(layer, frame).cloned().unwrap_or_else(|| {
            Cel::image(
                layer,
                frame,
                PixelBuffer::empty(self.sprite.width, self.sprite.height, ColorMode::Rgba),
            )
        })
    }

    /// Summary reported by `doc.info` / `doc.list`.
    pub fn info(&self, id: &str) -> Value {
        json!({
            "doc": id,
            "mode": "pixel",
            "width": self.sprite.width,
            "height": self.sprite.height,
            "layers": self.sprite.layers.len(),
            "frames": self.sprite.frames.len(),
            "tags": self.sprite.tags.len(),
            "palette_size": self.sprite.palette.len(),
            "active_layer": self.sprite.paint_target_layer(self.active_layer).map(|l| l.0),
            "active_frame": self.active_frame.0,
            "dirty": self.dirty,
            "path": self.path,
            "undo_depth": self.bus.undo_depth(),
            "redo_depth": self.bus.redo_depth(),
        })
    }
}

/// The documents of one agent session. In memory only until a tool saves
/// or exports.
#[derive(Debug, Default)]
pub struct Workspace {
    docs: Vec<(String, Doc)>,
    active: Option<String>,
    next_id: u32,
}

impl Workspace {
    /// An empty session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of open documents.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// `true` when no document is open.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Id of the active document.
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// Add `doc`, make it active and return its id.
    pub(crate) fn insert(&mut self, doc: Doc) -> String {
        self.next_id += 1;
        let id = format!("doc-{}", self.next_id);
        self.docs.push((id.clone(), doc));
        self.active = Some(id.clone());
        id
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Doc> {
        let pos = self.docs.iter().position(|(d, _)| d == id)?;
        let (_, doc) = self.docs.remove(pos);
        if self.active.as_deref() == Some(id) {
            self.active = self.docs.last().map(|(d, _)| d.clone());
        }
        Some(doc)
    }

    #[cfg(test)]
    pub(crate) fn get(&self, id: &str) -> Option<&Doc> {
        self.docs.iter().find(|(d, _)| d == id).map(|(_, d)| d)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &Doc)> {
        self.docs.iter().map(|(id, d)| (id.as_str(), d))
    }

    /// The id named by the `doc` argument, else the active document.
    pub(crate) fn resolve(&self, args: &Args) -> Result<String, AgentError> {
        match args.str("doc") {
            Some(id) => {
                if self.docs.iter().any(|(d, _)| d == id) {
                    Ok(id.to_owned())
                } else {
                    Err(AgentError::tool(format!(
                        "no open document with id `{id}`; doc.list shows the open documents"
                    )))
                }
            }
            None => self.active.clone().ok_or_else(|| {
                AgentError::tool("no document is open; call doc.new or doc.open first")
            }),
        }
    }

    pub(crate) fn doc(&self, args: &Args) -> Result<(String, &Doc), AgentError> {
        let id = self.resolve(args)?;
        let doc = self
            .docs
            .iter()
            .find(|(d, _)| *d == id)
            .map(|(_, d)| d)
            .ok_or_else(|| AgentError::tool(format!("no open document with id `{id}`")))?;
        Ok((id, doc))
    }

    pub(crate) fn doc_mut(&mut self, args: &Args) -> Result<(String, &mut Doc), AgentError> {
        let id = self.resolve(args)?;
        let doc = self
            .docs
            .iter_mut()
            .find(|(d, _)| *d == id)
            .map(|(_, d)| d)
            .ok_or_else(|| AgentError::tool(format!("no open document with id `{id}`")))?;
        Ok((id, doc))
    }
}
