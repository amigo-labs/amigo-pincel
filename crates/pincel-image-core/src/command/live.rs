//! Live objects (docs/specs/image-live-objects.md).
//!
//! A shape, a line or a gradient that stays editable after the drag that drew
//! it. The object is drawn into its layer at every change; its parameters ride
//! on the undo history as `LiveStep`s, so the object is live exactly while
//! the top undo steps are its steps. Commit folds them into one pixel step,
//! cancel removes them, and any other command commits first
//! ([`CommandBus::apply`]).

use std::any::Any;
use std::sync::Arc;

use super::{Command, CommandBus, Damage, SetPixels};
use crate::document::{Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::{Point, Rect};
use crate::tools::{
    DashPattern, Gradient, GradientChannel, GradientKind, Shape, ShapeMode, ShapeStyle, Shapes,
};

/// How far outside the canvas a live object's point may lie (AC-10).
pub const LIVE_REACH: f32 = 32_768.0;

/// The shapes of the Shapes group that a box (two opposite corners) defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxShape {
    /// An axis-aligned rectangle.
    Rectangle,
    /// A rectangle with rounded corners.
    RoundedRectangle,
    /// An ellipse inscribed in the box.
    Ellipse,
    /// A regular polygon inscribed in the shorter side of the box, centred.
    Polygon,
}

/// A live shape's geometry kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveShapeKind {
    /// A segment between two endpoints.
    Line,
    /// A box shape between two opposite corners.
    Box(BoxShape),
}

/// The parameters of a live Line or box shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveShape {
    /// Line or box shape.
    pub kind: LiveShapeKind,
    /// First endpoint or corner.
    pub a: Point,
    /// Second endpoint or opposite corner.
    pub b: Point,
    /// Mode, stroke width, colors, anti-alias and dash.
    pub style: ShapeStyle,
    /// Corner radius of a rounded rectangle (kept for every kind, AC-07).
    pub corner_radius: f32,
    /// Sides of a polygon (kept for every kind, AC-07).
    pub sides: u32,
}

/// The parameters of a live gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveGradient {
    /// Kind, channel, colors and opacity.
    pub gradient: Gradient,
    /// Drag start.
    pub a: Point,
    /// Drag end.
    pub b: Point,
}

/// What a live object draws: the parameters of its immediate draw command.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveParams {
    /// A Line or a box shape.
    Shape(LiveShape),
    /// A gradient.
    Gradient(LiveGradient),
}

/// A live object as the core reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveObject {
    /// The layer it is drawn into.
    pub layer: usize,
    /// Its current parameters.
    pub params: LiveParams,
}

/// The changed fields of a live-object edit; `None` keeps a field.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveEdit {
    /// New endpoints, box corners or gradient drag points.
    pub points: Option<[Point; 2]>,
    /// New shape kind (box shape to box shape only).
    pub shape: Option<LiveShapeKind>,
    /// New shape mode.
    pub mode: Option<ShapeMode>,
    /// New stroke width.
    pub stroke_width: Option<f32>,
    /// New dash pattern.
    pub dash: Option<DashPattern>,
    /// New anti-alias flag.
    pub anti_alias: Option<bool>,
    /// New corner radius.
    pub corner_radius: Option<f32>,
    /// New polygon sides.
    pub sides: Option<u32>,
    /// New outline color.
    pub stroke_color: Option<crate::Color>,
    /// New fill color.
    pub fill_color: Option<crate::Color>,
    /// New gradient kind.
    pub gradient_kind: Option<GradientKind>,
    /// New gradient channel.
    pub gradient_channel: Option<GradientChannel>,
    /// New gradient start color.
    pub gradient_from: Option<crate::Color>,
    /// New gradient end color.
    pub gradient_to: Option<crate::Color>,
    /// New gradient opacity.
    pub gradient_opacity: Option<f32>,
}

/// Parameter groups whose consecutive edits merge into one step (AC-03).
mod group {
    pub const GEOMETRY: u16 = 1;
    pub const SHAPE: u16 = 1 << 1;
    pub const MODE: u16 = 1 << 2;
    pub const STROKE_WIDTH: u16 = 1 << 3;
    pub const DASH: u16 = 1 << 4;
    pub const ANTI_ALIAS: u16 = 1 << 5;
    pub const CORNER_RADIUS: u16 = 1 << 6;
    pub const SIDES: u16 = 1 << 7;
    pub const COLORS: u16 = 1 << 8;
    pub const GRADIENT_KIND: u16 = 1 << 9;
    pub const GRADIENT_CHANNEL: u16 = 1 << 10;
    pub const GRADIENT_COLORS: u16 = 1 << 11;
    pub const GRADIENT_OPACITY: u16 = 1 << 12;
}

impl LiveEdit {
    /// The parameter groups this edit touches.
    fn groups(&self) -> u16 {
        let mut g = 0;
        let mut add = |set: bool, bit: u16| {
            if set {
                g |= bit;
            }
        };
        add(self.points.is_some(), group::GEOMETRY);
        add(self.shape.is_some(), group::SHAPE);
        add(self.mode.is_some(), group::MODE);
        add(self.stroke_width.is_some(), group::STROKE_WIDTH);
        add(self.dash.is_some(), group::DASH);
        add(self.anti_alias.is_some(), group::ANTI_ALIAS);
        add(self.corner_radius.is_some(), group::CORNER_RADIUS);
        add(self.sides.is_some(), group::SIDES);
        add(
            self.stroke_color.is_some() || self.fill_color.is_some(),
            group::COLORS,
        );
        add(self.gradient_kind.is_some(), group::GRADIENT_KIND);
        add(self.gradient_channel.is_some(), group::GRADIENT_CHANNEL);
        add(
            self.gradient_from.is_some() || self.gradient_to.is_some(),
            group::GRADIENT_COLORS,
        );
        add(self.gradient_opacity.is_some(), group::GRADIENT_OPACITY);
        g
    }

    fn touches_shape_fields(&self) -> bool {
        self.groups()
            & (group::SHAPE
                | group::MODE
                | group::STROKE_WIDTH
                | group::DASH
                | group::ANTI_ALIAS
                | group::CORNER_RADIUS
                | group::SIDES
                | group::COLORS)
            != 0
    }

    fn touches_gradient_fields(&self) -> bool {
        self.groups()
            & (group::GRADIENT_KIND
                | group::GRADIENT_CHANNEL
                | group::GRADIENT_COLORS
                | group::GRADIENT_OPACITY)
            != 0
    }
}

impl LiveParams {
    /// The two defining points: endpoints, box corners or drag points.
    pub fn points(&self) -> [Point; 2] {
        match self {
            LiveParams::Shape(s) => [s.a, s.b],
            LiveParams::Gradient(g) => [g.a, g.b],
        }
    }

    /// The history label of the object's tool ("Line", "Rectangle", …).
    pub fn label(&self) -> &'static str {
        match self {
            LiveParams::Shape(s) => s.shape().label(),
            LiveParams::Gradient(_) => "Gradient",
        }
    }

    /// Whether the geometry draws nothing: a zero-length Line or Gradient, a
    /// box of zero width or height.
    pub fn degenerate(&self) -> bool {
        let [a, b] = self.points();
        match self {
            LiveParams::Shape(LiveShape {
                kind: LiveShapeKind::Box(_),
                ..
            }) => a.x == b.x || a.y == b.y,
            _ => a == b,
        }
    }

    /// Clamps the style fields as the draw commands clamp them (AC-16).
    fn clamped(mut self) -> Self {
        match &mut self {
            LiveParams::Shape(s) => {
                s.style.stroke_width = s.style.stroke_width.clamp(1.0, 500.0);
                s.sides = s.sides.clamp(3, 100);
                s.corner_radius = s.corner_radius.max(0.0);
            }
            LiveParams::Gradient(g) => {
                g.gradient.opacity = g.gradient.opacity.clamp(0.0, 1.0);
            }
        }
        self
    }

    /// Whether every point lies within [`LIVE_REACH`] of the canvas (AC-10).
    fn within_reach(&self, doc: &Document) -> bool {
        let (w, h) = (doc.canvas.width() as f32, doc.canvas.height() as f32);
        self.points().iter().all(|p| {
            p.x.is_finite()
                && p.y.is_finite()
                && (-LIVE_REACH..=w + LIVE_REACH).contains(&p.x)
                && (-LIVE_REACH..=h + LIVE_REACH).contains(&p.y)
        })
    }

    /// These parameters with `edit` applied.
    fn edited(&self, edit: &LiveEdit) -> Result<LiveParams, DocumentError> {
        let mut out = self.clone();
        match &mut out {
            LiveParams::Shape(s) => {
                if edit.touches_gradient_fields() {
                    return Err(DocumentError::LiveEditMismatch);
                }
                if let Some(kind) = edit.shape {
                    match (s.kind, kind) {
                        (LiveShapeKind::Line, LiveShapeKind::Line)
                        | (LiveShapeKind::Box(_), LiveShapeKind::Box(_)) => s.kind = kind,
                        _ => return Err(DocumentError::LiveEditMismatch),
                    }
                }
                if let Some([a, b]) = edit.points {
                    s.a = a;
                    s.b = b;
                }
                let st = &mut s.style;
                st.mode = edit.mode.unwrap_or(st.mode);
                st.stroke_width = edit.stroke_width.unwrap_or(st.stroke_width);
                st.dash = edit.dash.unwrap_or(st.dash);
                st.anti_alias = edit.anti_alias.unwrap_or(st.anti_alias);
                st.stroke_color = edit.stroke_color.unwrap_or(st.stroke_color);
                st.fill_color = edit.fill_color.unwrap_or(st.fill_color);
                s.corner_radius = edit.corner_radius.unwrap_or(s.corner_radius);
                s.sides = edit.sides.unwrap_or(s.sides);
            }
            LiveParams::Gradient(g) => {
                if edit.touches_shape_fields() {
                    return Err(DocumentError::LiveEditMismatch);
                }
                if let Some([a, b]) = edit.points {
                    g.a = a;
                    g.b = b;
                }
                let gr = &mut g.gradient;
                gr.kind = edit.gradient_kind.unwrap_or(gr.kind);
                gr.channel = edit.gradient_channel.unwrap_or(gr.channel);
                gr.from = edit.gradient_from.unwrap_or(gr.from);
                gr.to = edit.gradient_to.unwrap_or(gr.to);
                gr.opacity = edit.gradient_opacity.unwrap_or(gr.opacity);
            }
        }
        Ok(out.clamped())
    }

    /// The immediate draw of these parameters on `doc`, or `None` when it
    /// draws nothing.
    fn draw(&self, layer: usize, doc: &Document) -> Option<SetPixels> {
        if self.degenerate() {
            return None;
        }
        match self {
            LiveParams::Shape(s) => Shapes::new(s.shape(), s.style).draw(layer, doc),
            LiveParams::Gradient(g) => g.gradient.draw(layer, g.a, g.b, doc),
        }
    }
}

impl LiveShape {
    /// The core [`Shape`] these parameters draw; a polygon is inscribed in
    /// the shorter side of the box, centred, as the editor builds it.
    pub fn shape(&self) -> Shape {
        let (a, b) = (self.a, self.b);
        match self.kind {
            LiveShapeKind::Line => Shape::Line { a, b },
            LiveShapeKind::Box(BoxShape::Rectangle) => Shape::Rectangle { a, b },
            LiveShapeKind::Box(BoxShape::RoundedRectangle) => Shape::RoundedRectangle {
                a,
                b,
                radius: self.corner_radius,
            },
            LiveShapeKind::Box(BoxShape::Ellipse) => Shape::Ellipse { a, b },
            LiveShapeKind::Box(BoxShape::Polygon) => Shape::Polygon {
                center: Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
                radius: (a.x - b.x).abs().min((a.y - b.y).abs()) / 2.0,
                sides: self.sides,
                rotation: 0.0,
            },
        }
    }
}

/// Parameters plus the layer rect they drew into (`None`: nothing).
#[derive(Debug, Clone)]
struct Drawn {
    params: LiveParams,
    rect: Option<Rect>,
}

/// One undo step of a live object: its begin (`before == None`) or an edit.
///
/// Both directions restore the pre-object pixels (`base`) over the rect the
/// object covers and draw the other parameters, so a step never depends on
/// the pixels a previous parameter set left behind (AC-02).
#[derive(Clone)]
pub(crate) struct LiveStep {
    layer: usize,
    base: Arc<ImageBuffer>,
    before: Option<Drawn>,
    after: Drawn,
}

impl LiveStep {
    /// The step that covers `older` then `newer` (a merge, AC-03, or a fold
    /// into the begin step, AC-14).
    fn joined(older: &LiveStep, newer: &LiveStep) -> LiveStep {
        LiveStep {
            layer: older.layer,
            base: Arc::clone(&older.base),
            before: older.before.clone(),
            after: newer.after.clone(),
        }
    }

    /// Joins the two oldest steps of a history whose bottom is a live
    /// object's begin step; `None` when they are not both live steps.
    pub(crate) fn fold(first: &dyn Command, second: &dyn Command) -> Option<Box<dyn Command>> {
        let a = first.as_any().downcast_ref::<LiveStep>()?;
        let b = second.as_any().downcast_ref::<LiveStep>()?;
        Some(Box::new(LiveStep::joined(a, b)))
    }
}

/// Whether `cmd` is a step of the live object.
pub(crate) fn is_live_step(cmd: &dyn Command) -> bool {
    cmd.as_any().is::<LiveStep>()
}

/// Restores `base` over `clear`, then draws `params` (if any) into `layer`.
/// Returns the rect the new draw covers.
fn render(
    doc: &mut Document,
    layer: usize,
    base: &ImageBuffer,
    clear: Option<Rect>,
    params: Option<&LiveParams>,
) -> Result<Option<Rect>, DocumentError> {
    let len = doc.layers.len();
    if let Some(r) = clear {
        let patch = base.copy_region(r)?;
        doc.layers
            .get_mut(layer)
            .ok_or(DocumentError::LayerIndexOutOfBounds { index: layer, len })?
            .pixels
            .paste_region(r, &patch)?;
    }
    let Some((rect, after)) = params
        .and_then(|p| p.draw(layer, doc))
        .and_then(SetPixels::into_single)
    else {
        return Ok(None);
    };
    doc.layers
        .get_mut(layer)
        .ok_or(DocumentError::LayerIndexOutOfBounds { index: layer, len })?
        .pixels
        .paste_region(rect, &after)?;
    Ok(Some(rect))
}

fn rect_damage(a: Option<Rect>, b: Option<Rect>) -> Damage {
    let mut d = Damage::None;
    for r in [a, b].into_iter().flatten() {
        d.add(Damage::rect(r));
    }
    d
}

impl Command for LiveStep {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let clear = self.before.as_ref().and_then(|d| d.rect);
        render(doc, self.layer, &self.base, clear, Some(&self.after.params))?;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let params = self.before.as_ref().map(|d| &d.params);
        render(doc, self.layer, &self.base, self.after.rect, params)?;
        Ok(())
    }

    fn label(&self) -> &str {
        self.after.params.label()
    }

    fn damage(&self) -> Damage {
        rect_damage(self.before.as_ref().and_then(|d| d.rect), self.after.rect)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// What the last live edit may merge with (AC-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MergeKey {
    /// The edits of one handle or body drag.
    Gesture(u64),
    /// Consecutive edits of one parameter group.
    Group(u16),
}

impl CommandBus {
    /// The live object, if any: kind, layer and parameters.
    pub fn live_object(&self) -> Option<LiveObject> {
        let top = self.top_live_step()?;
        Some(LiveObject {
            layer: top.layer,
            params: top.after.params.clone(),
        })
    }

    fn top_live_step(&self) -> Option<&LiveStep> {
        self.history
            .undo
            .last()
            .and_then(|c| c.as_any().downcast_ref::<LiveStep>())
    }

    /// Number of live steps on top of the undo history.
    fn live_depth(&self) -> usize {
        self.history
            .undo
            .iter()
            .rev()
            .take_while(|c| is_live_step(c.as_ref()))
            .count()
    }

    /// Begins a live object on `layer` (AC-01): commits the live object, if
    /// any, draws `params` and records one step. Returns `false` (nothing
    /// begun, no step) for degenerate geometry (AC-09).
    ///
    /// # Errors
    ///
    /// A missing layer, a locked layer or a group (`LayerLocked`), and
    /// points beyond [`LIVE_REACH`] are refused before anything changes.
    pub fn begin_live(&mut self, layer: usize, params: LiveParams) -> Result<bool, DocumentError> {
        let len = self.document.layers.len();
        if layer >= len {
            return Err(DocumentError::LayerIndexOutOfBounds { index: layer, len });
        }
        if super::spec::layer_locked(&self.document, layer) {
            return Err(DocumentError::LayerLocked { index: layer });
        }
        if !params.within_reach(&self.document) {
            return Err(DocumentError::LiveGeometryOutOfRange);
        }
        self.commit_if_live()?;
        if params.degenerate() {
            return Ok(false);
        }
        let params = params.clamped();
        let base = Arc::new(self.document.layers[layer].pixels.clone());
        let rect = render(&mut self.document, layer, &base, None, Some(&params))?;
        self.damage.add(rect_damage(rect, None));
        self.history.push(Box::new(LiveStep {
            layer,
            base,
            before: None,
            after: Drawn { params, rect },
        }));
        self.live_merge = None;
        Ok(true)
    }

    /// Edits the live object (AC-02): the layer shows the immediate draw of
    /// the edited parameters on the pre-object pixels. Edits of one
    /// `gesture`, or consecutive edits of one parameter group, form one
    /// step (AC-03). An edit that changes nothing adds no step.
    ///
    /// # Errors
    ///
    /// `NoLiveObject`, `LiveEditMismatch` (a field or shape change that
    /// does not fit the object's kind) and `LiveGeometryOutOfRange`; the
    /// object is unchanged after each.
    pub fn edit_live(
        &mut self,
        edit: &LiveEdit,
        gesture: Option<u64>,
    ) -> Result<(), DocumentError> {
        let top = self.top_live_step().ok_or(DocumentError::NoLiveObject)?;
        let params = top.after.params.edited(edit)?;
        if !params.within_reach(&self.document) {
            return Err(DocumentError::LiveGeometryOutOfRange);
        }
        if params == top.after.params {
            return Ok(());
        }
        let (layer, base, old) = (top.layer, Arc::clone(&top.base), top.after.clone());
        let rect = render(&mut self.document, layer, &base, old.rect, Some(&params))?;
        self.damage.add(rect_damage(old.rect, rect));
        let mut step = LiveStep {
            layer,
            base,
            before: Some(old),
            after: Drawn { params, rect },
        };
        let groups = edit.groups();
        let key = match gesture {
            Some(id) => Some(MergeKey::Gesture(id)),
            None if groups.count_ones() == 1 => Some(MergeKey::Group(groups)),
            None => None,
        };
        if key.is_some()
            && key == self.live_merge
            && let Some(prev) = self.history.undo.pop()
            && let Some(prev) = prev.as_any().downcast_ref::<LiveStep>()
        {
            step = LiveStep::joined(prev, &step);
        }
        self.history.push(Box::new(step));
        self.live_merge = key;
        Ok(())
    }

    /// Commits the live object (AC-05): its steps become one pixel step,
    /// or none when it leaves the layer as it was (AC-12).
    ///
    /// # Errors
    ///
    /// `NoLiveObject` when nothing is live.
    pub fn commit_live(&mut self) -> Result<(), DocumentError> {
        let n = self.live_depth();
        let top = self.top_live_step().ok_or(DocumentError::NoLiveObject)?;
        let (layer, base, drawn) = (top.layer, Arc::clone(&top.base), top.after.clone());
        let committed = match drawn.rect {
            Some(rect) => {
                let before = base.copy_region(rect)?;
                let after = self.document.layers[layer].pixels.copy_region(rect)?;
                (before != after)
                    .then(|| SetPixels::applied(layer, rect, before, after, drawn.params.label()))
            }
            None => None,
        };
        let keep = self.history.undo.len() - n;
        self.history.undo.truncate(keep);
        self.history.redo.clear();
        if let Some(cmd) = committed {
            self.history.undo.push(Box::new(cmd));
        }
        self.live_merge = None;
        Ok(())
    }

    /// Cancels the live object (AC-06): the layer shows the pre-object
    /// pixels and the object's steps leave the history; nothing can redo
    /// them.
    ///
    /// # Errors
    ///
    /// `NoLiveObject` when nothing is live.
    pub fn cancel_live(&mut self) -> Result<(), DocumentError> {
        let n = self.live_depth();
        let top = self.top_live_step().ok_or(DocumentError::NoLiveObject)?;
        let (layer, base, rect) = (top.layer, Arc::clone(&top.base), top.after.rect);
        render(&mut self.document, layer, &base, rect, None)?;
        self.damage.add(rect_damage(rect, None));
        let keep = self.history.undo.len() - n;
        self.history.undo.truncate(keep);
        self.history.redo.clear();
        self.live_merge = None;
        Ok(())
    }

    /// Drops the step of the handle drag `gesture` (Escape or Undo during
    /// the drag, docs/specs/image-live-objects-ui.md AC-11): the object
    /// returns to its parameters before the drag and stays live, and no
    /// redo step is kept. Returns `false` (nothing changes) when the top
    /// step is not that drag's.
    ///
    /// # Errors
    ///
    /// Only a failed redraw of the layer.
    pub fn cancel_live_gesture(&mut self, gesture: u64) -> Result<bool, DocumentError> {
        if self.live_merge != Some(MergeKey::Gesture(gesture)) || self.top_live_step().is_none() {
            return Ok(false);
        }
        if let Some(mut step) = self.history.undo.pop() {
            self.damage.add(step.damage());
            step.revert(&mut self.document)?;
        }
        self.live_merge = None;
        Ok(true)
    }

    /// Commits the live object if there is one (AC-08).
    pub(crate) fn commit_if_live(&mut self) -> Result<(), DocumentError> {
        if self.top_live_step().is_some() {
            self.commit_live()?;
        }
        Ok(())
    }

    /// Makes `index` the active layer; a change to a different layer
    /// commits the live object first (AC-08).
    ///
    /// # Errors
    ///
    /// `LayerIndexOutOfBounds`, before anything is committed.
    pub fn set_active_layer(&mut self, index: usize) -> Result<(), DocumentError> {
        let len = self.document.layers.len();
        if index >= len {
            return Err(DocumentError::LayerIndexOutOfBounds { index, len });
        }
        if index != self.document.active_layer_index() {
            self.commit_if_live()?;
        }
        self.document.set_active_layer(index)
    }
}
