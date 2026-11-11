//! The in-memory session: open documents, the active one, and the
//! per-document undo bus.

use pincel_core::{
    AnyCommand, Bus, Cel, CelMap, ColorMode, CommandBatch, Frame, FrameIndex, HistoryKind, Layer,
    LayerId, LayerKind, Palette, PixelBuffer, Sprite,
};
use serde_json::{Value, json};

use crate::AgentError;
use crate::args::Args;
use crate::fonts::{FontProvider, Fonts};
use crate::image::ImageDoc;

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
    /// The file a document without a file of its own was made from (a PNG
    /// opened with `mode: "pixel"`); `doc.save` suggests its stem.
    pub source_name: Option<String>,
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
            source_name: None,
            dirty: false,
        }
    }

    /// Run `cmds` as exactly one undo step (sealed on both sides, grouped
    /// in a [`CommandBatch`] when there are several). An empty list
    /// records nothing. A failing command leaves the document unchanged.
    /// The step's history kind is the first command's own kind; tools whose
    /// editor action it does not name use [`Doc::commit_as`].
    pub fn commit(&mut self, cmds: Vec<AnyCommand>) -> Result<(), AgentError> {
        self.commit_kind(None, cmds)
    }

    /// [`Doc::commit`] with the history kind of the editor action the tool
    /// mirrors (docs/specs/history-panel.md AC-15).
    pub fn commit_as(
        &mut self,
        kind: HistoryKind,
        cmds: Vec<AnyCommand>,
    ) -> Result<(), AgentError> {
        self.commit_kind(Some(kind), cmds)
    }

    fn commit_kind(
        &mut self,
        kind: Option<HistoryKind>,
        mut cmds: Vec<AnyCommand>,
    ) -> Result<(), AgentError> {
        let cmd = match cmds.len() {
            0 => return Ok(()),
            1 => cmds.remove(0),
            _ => CommandBatch::new(cmds).into(),
        };
        let kind = kind.unwrap_or_else(|| cmd.kind());
        self.bus.seal();
        let result = self
            .bus
            .execute_as(kind, cmd, &mut self.sprite, &mut self.cels);
        self.bus.seal();
        result.map_err(|e| AgentError::tool(e.to_string()))?;
        self.dirty = true;
        self.clamp_active();
        Ok(())
    }

    /// Run the commands `build` makes as one undo step, but only when they
    /// change the document: a dry run on a copy decides, so a call that
    /// changes nothing records no undo step (mcp-parity-structure AC-25).
    /// Returns whether a step was recorded; a failing command leaves the
    /// document unchanged.
    pub fn commit_if_changed(
        &mut self,
        build: impl Fn() -> Vec<AnyCommand>,
    ) -> Result<bool, AgentError> {
        self.commit_if_changed_kind(None, build)
    }

    /// [`Doc::commit_if_changed`] with the history kind `kind`, as
    /// [`Doc::commit_as`].
    pub fn commit_if_changed_as(
        &mut self,
        kind: HistoryKind,
        build: impl Fn() -> Vec<AnyCommand>,
    ) -> Result<bool, AgentError> {
        self.commit_if_changed_kind(Some(kind), build)
    }

    fn commit_if_changed_kind(
        &mut self,
        kind: Option<HistoryKind>,
        build: impl Fn() -> Vec<AnyCommand>,
    ) -> Result<bool, AgentError> {
        let mut sprite = self.sprite.clone();
        let mut cels = self.cels.clone();
        let mut cmds = build();
        let cmd = match cmds.len() {
            0 => return Ok(false),
            1 => cmds.remove(0),
            _ => CommandBatch::new(cmds).into(),
        };
        Bus::new()
            .execute(cmd, &mut sprite, &mut cels)
            .map_err(|e| AgentError::tool(e.to_string()))?;
        if sprite == self.sprite && cels == self.cels {
            return Ok(false);
        }
        self.commit_kind(kind, build())?;
        Ok(true)
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
                let layer = self.sprite.layer(id).ok_or_else(|| {
                    AgentError::tool(format!("argument `layer`: no layer with id {}", id.0))
                })?;
                let kind = match layer.kind {
                    LayerKind::Image => None,
                    LayerKind::Group => Some("a group"),
                    LayerKind::Tilemap { .. } => Some("a tilemap layer"),
                };
                if let Some(kind) = kind {
                    let hint = if kind == "a tilemap layer" {
                        "; edit its tiles with tile.set_pixels and its cells with tilemap.place"
                    } else {
                        ""
                    };
                    return Err(AgentError::tool(format!(
                        "layer {} ({:?}) is {kind}, not an image layer; pixel tools need an image layer{hint}",
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
        let g = self.sprite.metadata.grid_or_default();
        let grid = json!({ "x": g.x, "y": g.y, "width": g.width, "height": g.height });
        json!({
            "doc": id,
            "mode": "pixel",
            "width": self.sprite.width,
            "height": self.sprite.height,
            "layers": self.sprite.layers.len(),
            "frames": self.sprite.frames.len(),
            "tags": self.sprite.tags.len(),
            "palette_size": self.sprite.palette.len(),
            "color_mode": color_mode_name(self.sprite.color_mode),
            "slices": self.sprite.slices.len(),
            "tilesets": self.sprite.tilesets.len(),
            "grid": grid,
            "selection": self.selection_bounds(),
            "active_layer": self.sprite.paint_target_layer(self.active_layer).map(|l| l.0),
            "active_frame": self.active_frame.0,
            "dirty": self.dirty,
            "path": self.path,
            "undo_depth": self.bus.undo_depth(),
            "redo_depth": self.bus.redo_depth(),
        })
    }
}

impl Doc {
    /// The bounding box of the selected pixels, clipped to the canvas, as
    /// `{x, y, w, h}`; `null` without a selection (mcp-parity-structure
    /// AC-24).
    pub fn selection_bounds(&self) -> Value {
        let s = &self.sprite;
        let bounds = match &s.selection_mask {
            Some(mask) => mask.bounds(),
            None => s.selection.filter(|r| !r.is_empty()),
        };
        let clipped = bounds.and_then(|r| {
            let x0 = i64::from(r.x).max(0);
            let y0 = i64::from(r.y).max(0);
            let x1 = (i64::from(r.x) + i64::from(r.width)).min(i64::from(s.width));
            let y1 = (i64::from(r.y) + i64::from(r.height)).min(i64::from(s.height));
            (x1 > x0 && y1 > y0).then(|| json!({ "x": x0, "y": y0, "w": x1 - x0, "h": y1 - y0 }))
        });
        clipped.unwrap_or(Value::Null)
    }
}

/// The wire name of a Pixel color mode.
pub(crate) fn color_mode_name(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Rgba => "rgba",
        ColorMode::Indexed { .. } => "indexed",
        ColorMode::Grayscale => "grayscale",
    }
}

/// One open document of either mode.
#[derive(Debug)]
pub(crate) enum Entry {
    Pixel(Doc),
    Image(ImageDoc),
}

impl Entry {
    /// Summary reported by `doc.info` / `doc.list`.
    pub fn info(&self, id: &str) -> Value {
        match self {
            Self::Pixel(d) => d.info(id),
            Self::Image(d) => d.info(id),
        }
    }
}

/// The error for a tool called on a document of the other mode (AC-05).
fn wrong_mode(id: &str, is: &str, needs: &str) -> AgentError {
    AgentError::tool(format!(
        "document `{id}` is {is} document; this tool works on {needs} documents only (pincel://guide lists the tools of each mode)"
    ))
}

/// The documents of one agent session. In memory only until a tool saves
/// or exports.
#[derive(Debug, Default)]
pub struct Workspace {
    docs: Vec<(String, Entry)>,
    active: Option<String>,
    next_id: u32,
    fonts: Fonts,
}

impl Workspace {
    /// An empty session whose `draw.text` knows the bundled Liberation Sans
    /// only.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty session whose `draw.text` and `font.list` also offer the
    /// faces of `fonts` (docs/specs/system-fonts-mcp.md).
    pub fn with_fonts(fonts: impl FontProvider + 'static) -> Self {
        Self {
            fonts: Fonts::new(Box::new(fonts)),
            ..Self::default()
        }
    }

    /// The session's fonts.
    pub(crate) fn fonts_mut(&mut self) -> &mut Fonts {
        &mut self.fonts
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

    /// Add a Pixel document, make it active and return its id.
    pub(crate) fn insert(&mut self, doc: Doc) -> String {
        self.insert_entry(Entry::Pixel(doc))
    }

    /// Add an Image document, make it active and return its id.
    pub(crate) fn insert_image(&mut self, doc: ImageDoc) -> String {
        self.insert_entry(Entry::Image(doc))
    }

    fn insert_entry(&mut self, entry: Entry) -> String {
        self.next_id += 1;
        let id = format!("doc-{}", self.next_id);
        self.docs.push((id.clone(), entry));
        self.active = Some(id.clone());
        id
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Entry> {
        let pos = self.docs.iter().position(|(d, _)| d == id)?;
        let (_, doc) = self.docs.remove(pos);
        if self.active.as_deref() == Some(id) {
            self.active = self.docs.last().map(|(d, _)| d.clone());
        }
        Some(doc)
    }

    #[cfg(test)]
    pub(crate) fn get(&self, id: &str) -> Option<&Doc> {
        match self.entry(id)? {
            Entry::Pixel(d) => Some(d),
            Entry::Image(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, id: &str) -> Option<&mut Doc> {
        match self.entry_mut(id)? {
            Entry::Pixel(d) => Some(d),
            Entry::Image(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn get_image(&self, id: &str) -> Option<&ImageDoc> {
        match self.entry(id)? {
            Entry::Image(d) => Some(d),
            Entry::Pixel(_) => None,
        }
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.docs.iter().find(|(d, _)| d == id).map(|(_, e)| e)
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.docs.iter_mut().find(|(d, _)| d == id).map(|(_, e)| e)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &Entry)> {
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

    fn resolved(&self, args: &Args) -> Result<(String, &Entry), AgentError> {
        let id = self.resolve(args)?;
        let entry = self
            .entry(&id)
            .ok_or_else(|| AgentError::tool(format!("no open document with id `{id}`")))?;
        Ok((id, entry))
    }

    fn resolved_mut(&mut self, args: &Args) -> Result<(String, &mut Entry), AgentError> {
        let id = self.resolve(args)?;
        let entry = self
            .entry_mut(&id)
            .ok_or_else(|| AgentError::tool(format!("no open document with id `{id}`")))?;
        Ok((id, entry))
    }

    /// `doc.info` of the targeted document, either mode.
    pub(crate) fn info(&self, args: &Args) -> Result<Value, AgentError> {
        let (id, entry) = self.resolved(args)?;
        Ok(entry.info(&id))
    }

    /// `true` when the document the call targets is an Image document.
    pub(crate) fn is_image(&self, args: &Args) -> Result<bool, AgentError> {
        Ok(matches!(self.resolved(args)?.1, Entry::Image(_)))
    }

    /// The targeted Pixel document; an Image document is refused (AC-05).
    pub(crate) fn doc(&self, args: &Args) -> Result<(String, &Doc), AgentError> {
        match self.resolved(args)? {
            (id, Entry::Pixel(d)) => Ok((id, d)),
            (id, Entry::Image(_)) => Err(wrong_mode(&id, "an Image", "Pixel")),
        }
    }

    pub(crate) fn doc_mut(&mut self, args: &Args) -> Result<(String, &mut Doc), AgentError> {
        match self.resolved_mut(args)? {
            (id, Entry::Pixel(d)) => Ok((id, d)),
            (id, Entry::Image(_)) => Err(wrong_mode(&id, "an Image", "Pixel")),
        }
    }

    /// The targeted Image document; a Pixel document is refused (AC-05).
    pub(crate) fn image(&self, args: &Args) -> Result<(String, &ImageDoc), AgentError> {
        match self.resolved(args)? {
            (id, Entry::Image(d)) => Ok((id, d)),
            (id, Entry::Pixel(_)) => Err(wrong_mode(&id, "a Pixel", "Image")),
        }
    }

    pub(crate) fn image_mut(&mut self, args: &Args) -> Result<(String, &mut ImageDoc), AgentError> {
        match self.resolved_mut(args)? {
            (id, Entry::Image(d)) => Ok((id, d)),
            (id, Entry::Pixel(_)) => Err(wrong_mode(&id, "a Pixel", "Image")),
        }
    }
}
