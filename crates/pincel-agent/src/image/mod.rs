//! Image-mode documents and tools over `pincel-image-core`.
//!
//! Every edit builds the editor's own [`CommandSpec`] (or core command) and
//! runs it through [`run_command`], so a tool call leaves exactly the
//! pixels the editor's tool would. See docs/specs/mcp-headless-image.md.

pub(crate) mod doc;
pub(crate) mod draw;
pub(crate) mod layer;

use std::any::Any;
use std::fmt;

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::command::{Command, CommandBus, DEFAULT_UNDO_CAPACITY, HistoryKind};
use pincel_image_core::{Document, DocumentError};
use serde_json::{Value, json};

use crate::args::{Args, parse_hex};
use crate::{AgentError, Tool};

/// The Text tool's face: Liberation Sans, the font the editor ships
/// (Decision Log 2026-09-06), embedded so the headless binary needs no
/// font file.
pub(crate) const FONT: &[u8] =
    include_bytes!("../../../../ui/public/fonts/LiberationSans-Regular.ttf");

/// Largest Image document side, as in the editor (`MAX_CANVAS_DIM`).
pub(crate) const MAX_SIDE: u32 = 16_384;
/// Largest Image document area, as in the editor (`MAX_CANVAS_AREA`).
pub(crate) const MAX_AREA: u64 = 8192 * 8192;

/// Image-only tools, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    draw::STROKE,
    draw::SHAPE,
    draw::TEXT,
    draw::GRADIENT,
    draw::RECOLOR,
    draw::SMUDGE,
    draw::ADJUST_APPLY,
    doc::EXPORT_JPEG,
    doc::EXPORT_WEBP,
    doc::EXPORT_BMP,
    doc::EXPORT_TIFF,
];

/// Refuse sizes above the editor's Image limits (AC-21).
pub(crate) fn check_size(width: u32, height: u32) -> Result<(), AgentError> {
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(AgentError::tool(format!(
            "{width}×{height} px exceeds the Image document limit of {MAX_SIDE} px per side"
        )));
    }
    if u64::from(width) * u64::from(height) > MAX_AREA {
        return Err(AgentError::tool(format!(
            "{width}×{height} px exceeds the Image document area limit of 8192 × 8192 px (64 Mpx)"
        )));
    }
    Ok(())
}

pub(crate) fn core_err(e: DocumentError) -> AgentError {
    AgentError::tool(e.to_string())
}

/// A color argument of an Image tool: `#RRGGBB` or `#RRGGBBAA`.
pub(crate) fn color_arg(args: &Args, key: &str) -> Result<[u8; 4], AgentError> {
    let v = args
        .get(key)
        .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))?;
    match v.as_str().and_then(parse_hex) {
        Some(c) => Ok([c.r, c.g, c.b, c.a]),
        None => Err(AgentError::tool(format!(
            "argument `{key}`: malformed color {v}; Image documents take \"#RRGGBB\" or \"#RRGGBBAA\" (palette indices are for Pixel documents)"
        ))),
    }
}

/// Several core commands applied and undone as one history entry of the
/// history kind `kind`.
pub(crate) struct Batch {
    pub cmds: Vec<Box<dyn Command>>,
    pub kind: HistoryKind,
}

impl Command for Batch {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        for i in 0..self.cmds.len() {
            if let Err(e) = self.cmds[i].apply(doc) {
                // Roll back what already ran so a failed call changes
                // nothing; the original error is the one worth reporting.
                for done in self.cmds[..i].iter_mut().rev() {
                    done.revert(doc)?;
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        for cmd in self.cmds.iter_mut().rev() {
            cmd.revert(doc)?;
        }
        Ok(())
    }

    fn kind(&self) -> HistoryKind {
        self.kind
    }

    fn undo_bytes(&self) -> usize {
        self.cmds.iter().map(|c| c.undo_bytes()).sum()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// One open Image document.
pub(crate) struct ImageDoc {
    pub bus: CommandBus,
    scratch: StrokeScratch,
    /// Stable agent layer ids by core layer UUID; never shrinks, so a
    /// layer restored by undo keeps its id.
    ids: Vec<(u128, u32)>,
    next_stroke: u64,
    /// The `.ora` file the document was opened from or last saved to.
    pub path: Option<String>,
    /// The flat image it was opened from and that file's format (AC-20).
    pub source: Option<(String, &'static str)>,
    /// The quality of the last JPEG `doc.save` (mcp-parity-export AC-18).
    pub jpeg_quality: Option<u8>,
    /// Changed since it was opened, created or last saved.
    pub dirty: bool,
}

impl fmt::Debug for ImageDoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageDoc")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("layers", &self.bus.document.layer_count())
            .field("path", &self.path)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl ImageDoc {
    pub fn new(
        document: Document,
        path: Option<String>,
        source: Option<(String, &'static str)>,
    ) -> Self {
        let mut doc = Self {
            bus: CommandBus::new(document),
            scratch: StrokeScratch::default(),
            ids: Vec::new(),
            next_stroke: 0,
            path,
            source,
            jpeg_quality: None,
            dirty: false,
        };
        doc.sync_ids();
        doc
    }

    pub fn document(&self) -> &Document {
        &self.bus.document
    }

    pub fn width(&self) -> u32 {
        self.bus.document.canvas.width()
    }

    pub fn height(&self) -> u32 {
        self.bus.document.canvas.height()
    }

    /// Give every layer without an agent id the next one.
    pub fn sync_ids(&mut self) {
        for layer in self.bus.document.layers() {
            let key = layer.id.as_u128();
            if !self.ids.iter().any(|(k, _)| *k == key) {
                let next = self.ids.len() as u32;
                self.ids.push((key, next));
            }
        }
    }

    /// The agent id of the layer at `index`.
    pub fn layer_id(&self, index: usize) -> u32 {
        let key = self
            .bus
            .document
            .layers()
            .get(index)
            .map(|l| l.id.as_u128());
        self.ids
            .iter()
            .find(|(k, _)| Some(*k) == key)
            .map_or(index as u32, |(_, id)| *id)
    }

    /// The stack index of the layer with agent id `id`.
    pub fn layer_index(&self, id: u32) -> Result<usize, AgentError> {
        let key = self.ids.iter().find(|(_, i)| *i == id).map(|(k, _)| *k);
        self.bus
            .document
            .layers()
            .iter()
            .position(|l| Some(l.id.as_u128()) == key)
            .ok_or_else(|| {
                AgentError::tool(format!("no layer with id {id}; layer.list shows the ids"))
            })
    }

    /// The stack index of the layer with agent id `id`, naming the
    /// argument `key` in the error (mcp-parity-structure AC-27).
    pub fn layer_arg(&self, id: u32, key: &str) -> Result<usize, AgentError> {
        self.layer_index(id).map_err(|_| {
            AgentError::tool(format!(
                "argument `{key}`: no layer with id {id}; layer.list shows the ids"
            ))
        })
    }

    /// The layer an edit targets: the `layer` argument, else the active
    /// layer. Refuses a group (it has no pixels) and a locked layer.
    pub fn target(&self, args: &Args) -> Result<usize, AgentError> {
        let index = match args.u32("layer")? {
            Some(id) => self.layer_arg(id, "layer")?,
            None => self.bus.document.active_layer_index(),
        };
        self.check_paintable(index)?;
        Ok(index)
    }

    /// Refuse a pixel edit on a group (mcp-parity-structure AC-13) or on a
    /// locked layer.
    pub fn check_paintable(&self, index: usize) -> Result<(), AgentError> {
        let doc = &self.bus.document;
        if doc.layers().get(index).is_some_and(|l| l.is_group()) {
            return Err(AgentError::tool(format!(
                "layer {} ({:?}) is a group, which has no pixels; pass the id of an image layer inside it (layer.list)",
                self.layer_id(index),
                doc.layers()[index].name
            )));
        }
        self.check_unlocked(index)
    }

    /// Refuse to edit a locked layer or a layer inside a locked group.
    pub fn check_unlocked(&self, index: usize) -> Result<(), AgentError> {
        if self.bus.document.effectively_locked(index) {
            let name = self.bus.document.layers()[index].name.as_str();
            return Err(AgentError::tool(format!(
                "layer {} ({name:?}) is locked (or inside a locked group); unlock it with layer.set {{\"locked\": false}}",
                self.layer_id(index)
            )));
        }
        Ok(())
    }

    /// A stroke id no earlier call used, so two calls never merge into one
    /// undo step.
    pub fn next_stroke(&mut self) -> u64 {
        self.next_stroke += 1;
        self.next_stroke
    }

    /// Run one editor command as one undo step. Returns whether a step was
    /// recorded (an edit that misses the canvas records none).
    pub fn run(&mut self, spec: CommandSpec) -> Result<bool, AgentError> {
        self.check_spec(&spec)?;
        let before = self.bus.history.undo_depth();
        run_command(&mut self.bus, spec, &mut self.scratch, &[FONT]).map_err(core_err)?;
        Ok(self.recorded(before))
    }

    /// Refuse a pixel edit the editor refuses: on a locked layer, or on a
    /// group unless the edit moves or turns the group's content.
    fn check_spec(&self, spec: &CommandSpec) -> Result<(), AgentError> {
        if let Some(index) = spec.edited_layer() {
            let moves_content = matches!(
                spec,
                CommandSpec::TranslateLayer { .. }
                    | CommandSpec::TransformLayer { .. }
                    | CommandSpec::RotateLayer90 { .. }
            );
            let doc = &self.bus.document;
            if moves_content && doc.layers().get(index).is_some_and(|l| l.is_group()) {
                // Moving or turning a group turns its subtree, unless a
                // layer in it is locked (image-layer-groups AC-18).
                for i in index..doc.subtree_end(index) {
                    self.check_unlocked(i)?;
                }
            } else {
                self.check_paintable(index)?;
            }
        }
        Ok(())
    }

    /// Run the editor command `build` makes, but only when it changes the
    /// document: a dry run on a copy decides, so a call that changes
    /// nothing records no undo step (mcp-parity-selection-transform
    /// AC-36). Returns whether a step was recorded.
    pub fn run_if_changed(&mut self, build: impl Fn() -> CommandSpec) -> Result<bool, AgentError> {
        self.check_spec(&build())?;
        let mut copy = CommandBus::new(self.bus.document.clone());
        run_command(&mut copy, build(), &mut StrokeScratch::default(), &[FONT])
            .map_err(core_err)?;
        let mut before = self.bus.document.clone();
        let mut after = copy.document;
        before
            .set_active_layer(0)
            .expect("a document has at least one layer");
        after
            .set_active_layer(0)
            .expect("a document has at least one layer");
        if before == after {
            return Ok(false);
        }
        self.run(build())
    }

    /// Apply core commands as one undo step.
    pub fn apply(&mut self, cmds: Vec<Box<dyn Command>>) -> Result<bool, AgentError> {
        self.apply_kind(None, cmds)
    }

    /// [`ImageDoc::apply`] with the history kind of the editor action the
    /// tool mirrors (docs/specs/history-panel.md AC-15).
    pub fn apply_as(
        &mut self,
        kind: HistoryKind,
        cmds: Vec<Box<dyn Command>>,
    ) -> Result<bool, AgentError> {
        self.apply_kind(Some(kind), cmds)
    }

    /// Several commands become one [`Batch`] of the first command's kind
    /// unless `kind` names one.
    fn apply_kind(
        &mut self,
        kind: Option<HistoryKind>,
        mut cmds: Vec<Box<dyn Command>>,
    ) -> Result<bool, AgentError> {
        let cmd = match (cmds.len(), kind) {
            (0, _) => return Ok(false),
            (1, None) => cmds.remove(0),
            (_, kind) => {
                let kind = kind.unwrap_or_else(|| cmds[0].kind());
                Box::new(Batch { cmds, kind })
            }
        };
        let before = self.bus.history.undo_depth();
        self.bus.apply(cmd).map_err(core_err)?;
        Ok(self.recorded(before))
    }

    /// Apply the commands `build` makes as one undo step, but only when
    /// they change the document: a dry run on a copy decides, so a call
    /// that changes nothing records no undo step (mcp-parity-structure
    /// AC-25). A change of the active layer alone does not count. Returns
    /// whether a step was recorded; a failing command changes nothing.
    pub fn apply_if_changed(
        &mut self,
        build: impl Fn() -> Vec<Box<dyn Command>>,
    ) -> Result<bool, AgentError> {
        let mut copy = self.bus.document.clone();
        let mut cmd = Batch {
            cmds: build(),
            kind: HistoryKind::DrawPixels,
        };
        cmd.apply(&mut copy).map_err(core_err)?;
        let mut before = self.bus.document.clone();
        before
            .set_active_layer(0)
            .expect("a document has at least one layer");
        copy.set_active_layer(0)
            .expect("a document has at least one layer");
        if copy == before {
            return Ok(false);
        }
        self.apply(build())
    }

    fn recorded(&mut self, before: usize) -> bool {
        // A full history drops its oldest entry on push, so the depth stays
        // put; count that case as recorded.
        let after = self.bus.history.undo_depth();
        let changed = after != before || before == DEFAULT_UNDO_CAPACITY;
        if changed {
            self.dirty = true;
            self.sync_ids();
        }
        changed
    }

    /// Summary reported by `doc.info` / `doc.list`.
    pub fn info(&self, id: &str) -> Value {
        let doc = &self.bus.document;
        let selection = doc
            .selection
            .as_ref()
            .and_then(|s| s.bounding_box())
            .map(|r| json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h }));
        json!({
            "doc": id,
            "mode": "image",
            "width": self.width(),
            "height": self.height(),
            "layers": doc.layer_count(),
            "color_mode": "rgba",
            "active_layer": self.layer_id(doc.active_layer_index()),
            "selection": selection,
            "dirty": self.dirty,
            "path": self.path,
            "opened_from": self.source.as_ref().map(|(p, _)| p),
            "undo_depth": self.bus.history.undo_depth(),
            "redo_depth": self.bus.history.redo_depth(),
        })
    }
}
