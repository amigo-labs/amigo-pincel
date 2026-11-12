//! The live selection transform of the Move tools' handles
//! (docs/specs/image-transform-handles.md): the selection's content (Move
//! Selected Pixels) or outline (Move Selection) is lifted once, and every
//! change of the box, angle, pivot or quality re-applies the transform from
//! that lifted original, so the result depends only on the final settings
//! (AC-11) and content beyond the canvas is kept while live (AC-13). It is a
//! live object in the sense of `image-live-objects`: each change is an undo
//! step, commit folds them into one, cancel removes them, and any other
//! command commits first. Its settings live on the command bus, never in the
//! document, so files and autosaves carry only the applied pixels (AC-18).

use std::any::Any;
use std::sync::Arc;

use pincel_effects::EffectImage;
use pincel_effects::transform::{Affine, Quality, resample};

use super::{Command, CommandBus, Damage, HistoryKind, live::MergeKey};
use crate::document::{Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::Rect;
use crate::selection::SelectionMask;

/// Where the box of a live transform is and how its content is resampled.
///
/// The box is the selection's bounds when the transform began, scaled to
/// `size` along its own axes, turned by `angle` and centred on `center`
/// (canvas pixel-edge coordinates). Only these fields decide the result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformSettings {
    /// Centre of the box on the canvas.
    pub center: (f64, f64),
    /// Width and height of the box (at least 1 px each).
    pub size: (f64, f64),
    /// Turn of the box in degrees, positive clockwise on screen.
    pub angle: f64,
    /// The pivot relative to the box: `(0, 0)` its top-left corner,
    /// `(0.5, 0.5)` its centre, in the box's own (turned) frame. It decides
    /// only where later rotations turn about, never a pixel (AC-06).
    pub pivot: (f64, f64),
    /// Resampling quality (AC-12).
    pub quality: Quality,
}

impl TransformSettings {
    /// The settings that leave a box with the bounds `center` / `size` as
    /// it is: no turn, pivot at the centre, Bilinear.
    pub fn identity_at(center: (f64, f64), size: (f64, f64)) -> Self {
        Self {
            center,
            size,
            angle: 0.0,
            pivot: (0.5, 0.5),
            quality: Quality::Bilinear,
        }
    }

    /// The identity settings for `doc`'s selection, or `None` without a
    /// selection that selects a pixel (AC-17).
    pub fn for_selection(doc: &Document) -> Option<Self> {
        let r = doc.selection.as_ref()?.bounding_box()?;
        Some(Self::identity_at(
            (
                f64::from(r.x) + f64::from(r.w) / 2.0,
                f64::from(r.y) + f64::from(r.h) / 2.0,
            ),
            (f64::from(r.w), f64::from(r.h)),
        ))
    }

    /// These settings with the box scaled by `(fx, fy)` about its centre.
    pub fn scaled_by(self, fx: f64, fy: f64) -> Self {
        Self {
            size: ((self.size.0 * fx).max(1.0), (self.size.1 * fy).max(1.0)),
            ..self
        }
    }

    /// The same box and angle (pivot and quality aside).
    fn same_geometry(&self, other: &Self) -> bool {
        self.center == other.center && self.size == other.size && self.angle == other.angle
    }
}

/// A live transform as the core reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveTransform {
    /// The layer it lifted from (Move Selected Pixels) or sits on.
    pub layer: usize,
    /// `true` for Move Selected Pixels, `false` for Move Selection.
    pub pixels: bool,
    /// The selection's bounds when it began.
    pub bounds: Rect,
    /// Its current settings.
    pub settings: TransformSettings,
}

/// What a transform keeps from its start, shared by all its steps.
struct Base {
    layer: usize,
    pixels: bool,
    bounds: Rect,
    /// The layer before the transform (Move Selected Pixels).
    original: Option<ImageBuffer>,
    /// The layer with the lifted pixels taken out (Move Selected Pixels).
    hole: Option<ImageBuffer>,
    /// The lifted pixels inside `bounds`, alpha scaled by coverage.
    patch: Option<EffectImage>,
    /// The selection before the transform.
    mask: SelectionMask,
    /// The selection's coverage inside `bounds` as alpha.
    mask_patch: EffectImage,
}

impl Base {
    fn identity(&self) -> TransformSettings {
        let b = self.bounds;
        TransformSettings::identity_at(
            (
                f64::from(b.x) + f64::from(b.w) / 2.0,
                f64::from(b.y) + f64::from(b.h) / 2.0,
            ),
            (f64::from(b.w), f64::from(b.h)),
        )
    }

    /// The map from patch-local coordinates (`(0, 0)` = the bounds' corner)
    /// to the canvas.
    fn forward(&self, s: &TransformSettings) -> Affine {
        let (bw, bh) = (f64::from(self.bounds.w), f64::from(self.bounds.h));
        let (sx, sy) = (s.size.0 / bw, s.size.1 / bh);
        let (sin, cos) = sin_cos_degrees(s.angle);
        let (a, b, d, e) = (cos * sx, -sin * sy, sin * sx, cos * sy);
        Affine {
            a,
            b,
            c: s.center.0 - (a * bw / 2.0 + b * bh / 2.0),
            d,
            e,
            f: s.center.1 - (d * bw / 2.0 + e * bh / 2.0),
        }
    }

    /// The canvas rect the transformed box covers, clipped to the canvas;
    /// `None` when it lies wholly outside.
    fn target(&self, s: &TransformSettings, canvas: (u32, u32)) -> Option<Rect> {
        let f = self.forward(s);
        let (bw, bh) = (f64::from(self.bounds.w), f64::from(self.bounds.h));
        let corners = [(0.0, 0.0), (bw, 0.0), (0.0, bh), (bw, bh)].map(|(u, v)| f.apply(u, v));
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in corners {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let r = Rect::new(
            x0.floor() as i32,
            y0.floor() as i32,
            (x1.ceil() - x0.floor()).max(0.0) as u32,
            (y1.ceil() - y0.floor()).max(0.0) as u32,
        );
        r.intersect(Rect::new(0, 0, canvas.0, canvas.1))
    }

    /// `src` (patch-local) resampled into `target` under `s`.
    fn resampled(&self, src: &EffectImage, s: &TransformSettings, target: Rect) -> Option<Vec<u8>> {
        let inv = self.forward(s).inverse()?;
        let (rx, ry) = (f64::from(target.x), f64::from(target.y));
        let shifted = Affine {
            c: inv.a * rx + inv.b * ry + inv.c,
            f: inv.d * rx + inv.e * ry + inv.f,
            ..inv
        };
        Some(resample(src, target.w, target.h, &shifted, s.quality).into_raw())
    }

    /// Puts the layer and the selection in the state `s` shows (`None`: the
    /// state before the transform), first restoring `clear` from the hole.
    /// Returns the rect the content now covers.
    fn render(
        &self,
        doc: &mut Document,
        clear: Option<Rect>,
        s: Option<&TransformSettings>,
    ) -> Result<Option<Rect>, DocumentError> {
        let canvas = (doc.canvas.width(), doc.canvas.height());
        let identity = s.is_none_or(|s| s.same_geometry(&self.identity()));
        if identity {
            if let Some(original) = &self.original {
                let len = doc.layers.len();
                let layer =
                    doc.layers
                        .get_mut(self.layer)
                        .ok_or(DocumentError::LayerIndexOutOfBounds {
                            index: self.layer,
                            len,
                        })?;
                for r in [clear, Some(self.bounds)].into_iter().flatten() {
                    if let Some(r) = r.intersect(Rect::new(0, 0, canvas.0, canvas.1)) {
                        layer.pixels.paste_region(r, &original.copy_region(r)?)?;
                    }
                }
            }
            doc.selection = Some(self.mask.clone());
            return Ok(Some(self.bounds));
        }
        let s = s.expect("non-identity settings");
        let target = self.target(s, canvas);
        if let (Some(hole), Some(patch)) = (&self.hole, &self.patch) {
            let len = doc.layers.len();
            let layer =
                doc.layers
                    .get_mut(self.layer)
                    .ok_or(DocumentError::LayerIndexOutOfBounds {
                        index: self.layer,
                        len,
                    })?;
            for r in [clear, Some(self.bounds)].into_iter().flatten() {
                if let Some(r) = r.intersect(Rect::new(0, 0, canvas.0, canvas.1)) {
                    layer.pixels.paste_region(r, &hole.copy_region(r)?)?;
                }
            }
            if let Some(t) = target
                && let Some(src) = self.resampled(patch, s, t)
            {
                let mut dst = layer.pixels.copy_region(t)?.data().to_vec();
                source_over(&mut dst, &src);
                layer
                    .pixels
                    .paste_region(t, &ImageBuffer::from_raw(t.w, t.h, dst)?)?;
            }
        }
        let mut mask = SelectionMask::new_empty(canvas.0, canvas.1);
        if let Some(t) = target
            && let Some(cov) = self.resampled(&self.mask_patch, s, t)
        {
            for y in 0..t.h {
                for x in 0..t.w {
                    let a = cov[((y * t.w + x) * 4 + 3) as usize];
                    if a > 0 {
                        mask.set(t.x as u32 + x, t.y as u32 + y, a);
                    }
                }
            }
        }
        doc.selection = Some(mask);
        Ok(target)
    }
}

/// Straight-alpha source-over of `src` onto `dst` (same size, RGBA8).
fn source_over(dst: &mut [u8], src: &[u8]) {
    let (dst, _) = dst.as_chunks_mut::<4>();
    let (src, _) = src.as_chunks::<4>();
    for (d, s) in dst.iter_mut().zip(src) {
        let sa = f32::from(s[3]) / 255.0;
        if sa <= 0.0 {
            continue;
        }
        if s[3] == 255 {
            d.copy_from_slice(s);
            continue;
        }
        let da = f32::from(d[3]) / 255.0;
        let oa = sa + da * (1.0 - sa);
        if oa <= 0.0 {
            d.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        for i in 0..3 {
            let c = (f32::from(s[i]) * sa + f32::from(d[i]) * da * (1.0 - sa)) / oa;
            d[i] = c.round().clamp(0.0, 255.0) as u8;
        }
        d[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

/// `(sin, cos)` of `degrees`, exact at multiples of 90°.
fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    let r = degrees.rem_euclid(360.0);
    match r {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        _ => {
            let a = r.to_radians();
            (libm::sin(a), libm::cos(a))
        }
    }
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
            let (x1, y1) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
            Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
        }
        (a, None) => a,
        (None, b) => b,
    }
}

/// The settings a step shows, and the rect its content covered.
#[derive(Clone, Copy)]
struct Shown {
    settings: TransformSettings,
    rect: Option<Rect>,
}

/// One undo step of a live transform: its begin (`before == None`) or a
/// change. Both directions re-apply from the lifted original.
#[derive(Clone)]
pub(crate) struct TransformStep {
    base: Arc<Base>,
    before: Option<Shown>,
    after: Shown,
}

impl TransformStep {
    fn joined(older: &TransformStep, newer: &TransformStep) -> TransformStep {
        TransformStep {
            base: Arc::clone(&older.base),
            before: older.before,
            after: newer.after,
        }
    }

    /// Joins the two oldest steps of a history whose bottom begins a live
    /// transform (`image-live-objects` AC-14 for transforms, AC-22).
    pub(crate) fn fold(first: &dyn Command, second: &dyn Command) -> Option<Box<dyn Command>> {
        let a = first.as_any().downcast_ref::<TransformStep>()?;
        let b = second.as_any().downcast_ref::<TransformStep>()?;
        Some(Box::new(TransformStep::joined(a, b)))
    }

    fn kind_of(base: &Base) -> HistoryKind {
        if base.pixels {
            HistoryKind::MoveSelection
        } else {
            HistoryKind::MoveSelectionMask
        }
    }
}

/// Whether `cmd` is a step of a live transform.
pub(crate) fn is_transform_step(cmd: &dyn Command) -> bool {
    cmd.as_any().is::<TransformStep>()
}

impl Command for TransformStep {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let clear = self.before.and_then(|b| b.rect);
        self.base.render(doc, clear, Some(&self.after.settings))?;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let before = self.before.map(|b| b.settings);
        self.base.render(doc, self.after.rect, before.as_ref())?;
        Ok(())
    }

    fn kind(&self) -> HistoryKind {
        Self::kind_of(&self.base)
    }

    fn damage(&self) -> Damage {
        if !self.base.pixels {
            return Damage::None;
        }
        let mut d = Damage::rect(self.base.bounds);
        for r in [self.before.and_then(|b| b.rect), self.after.rect]
            .into_iter()
            .flatten()
        {
            d.add(Damage::rect(r));
        }
        d
    }

    fn changes_selection(&self) -> bool {
        true
    }

    /// The lifted layer, its hole and the patches are held once, by the
    /// begin step.
    fn undo_bytes(&self) -> usize {
        if self.before.is_some() {
            return 0;
        }
        let b = &self.base;
        b.original.as_ref().map_or(0, |o| o.data().len())
            + b.hole.as_ref().map_or(0, |h| h.data().len())
            + b.patch.as_ref().map_or(0, |p| p.data().len())
            + b.mask.data().len()
            + b.mask_patch.data().len()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The step a committed transform leaves (AC-09): the changed pixels and
/// the outline before and after.
struct TransformCommit {
    layer: usize,
    pixels: Option<(Rect, ImageBuffer, ImageBuffer)>,
    mask_before: SelectionMask,
    mask_after: Option<SelectionMask>,
    kind: HistoryKind,
}

impl Command for TransformCommit {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if let Some((rect, _, after)) = &self.pixels {
            doc.layers[self.layer].pixels.paste_region(*rect, after)?;
        }
        doc.selection = self.mask_after.clone();
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if let Some((rect, before, _)) = &self.pixels {
            doc.layers[self.layer].pixels.paste_region(*rect, before)?;
        }
        doc.selection = Some(self.mask_before.clone());
        Ok(())
    }

    fn kind(&self) -> HistoryKind {
        self.kind
    }

    fn damage(&self) -> Damage {
        self.pixels
            .as_ref()
            .map_or(Damage::None, |(r, _, _)| Damage::rect(*r))
    }

    fn changes_selection(&self) -> bool {
        true
    }

    fn undo_bytes(&self) -> usize {
        self.pixels
            .as_ref()
            .map_or(0, |(_, b, a)| b.data().len() + a.data().len())
            + self.mask_before.data().len()
            + self.mask_after.as_ref().map_or(0, |m| m.data().len())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The merge group of quality-only changes (AC-07).
const QUALITY_GROUP: u32 = 1 << 30;

/// Lifts the selected pixels of `layer` by coverage: the patch inside
/// `bounds` and the layer with them taken out.
fn lift(src: &ImageBuffer, mask: &SelectionMask, bounds: Rect) -> (EffectImage, ImageBuffer) {
    let mut hole = src.clone();
    let mut patch = Vec::with_capacity(bounds.w as usize * bounds.h as usize * 4);
    for y in bounds.y..bounds.bottom() {
        for x in bounds.x..bounds.right() {
            let (ux, uy) = (x as u32, y as u32);
            let cov = u32::from(mask.coverage(ux, uy));
            let c = src
                .get_pixel(ux, uy)
                .unwrap_or(crate::color::Color::TRANSPARENT);
            if cov == 0 {
                patch.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let lifted = ((u32::from(c.a) * cov + 127) / 255) as u8;
            if lifted == 0 {
                patch.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                patch.extend_from_slice(&[c.r, c.g, c.b, lifted]);
            }
            let left = ((u32::from(c.a) * (255 - cov) + 127) / 255) as u8;
            hole.set_pixel(
                ux,
                uy,
                if left == 0 {
                    crate::color::Color::TRANSPARENT
                } else {
                    crate::color::Color::rgba(c.r, c.g, c.b, left)
                },
            );
        }
    }
    let patch = EffectImage::from_rgba8(bounds.w, bounds.h, patch).expect("patch size");
    (patch, hole)
}

fn mask_patch(mask: &SelectionMask, bounds: Rect) -> EffectImage {
    let mut data = Vec::with_capacity(bounds.w as usize * bounds.h as usize * 4);
    for y in bounds.y..bounds.bottom() {
        for x in bounds.x..bounds.right() {
            data.extend_from_slice(&[255, 255, 255, mask.coverage(x as u32, y as u32)]);
        }
    }
    EffectImage::from_rgba8(bounds.w, bounds.h, data).expect("mask patch size")
}

impl CommandBus {
    fn top_transform_step(&self) -> Option<&TransformStep> {
        self.history
            .undo
            .last()
            .and_then(|c| c.as_any().downcast_ref::<TransformStep>())
    }

    fn transform_depth(&self) -> usize {
        self.history
            .undo
            .iter()
            .rev()
            .take_while(|c| is_transform_step(c.as_ref()))
            .count()
    }

    /// The live transform, if any.
    pub fn live_transform(&self) -> Option<LiveTransform> {
        let top = self.top_transform_step()?;
        Some(LiveTransform {
            layer: top.base.layer,
            pixels: top.base.pixels,
            bounds: top.base.bounds,
            settings: top.after.settings,
        })
    }

    /// Begins a live transform of the selection on `layer` — its pixels
    /// (`pixels`, Move Selected Pixels) or its outline only (Move Selection)
    /// — at `settings`, as one undo step. Commits any other live object
    /// first. Returns `false` (nothing begun) without a selection that
    /// selects a pixel (AC-17).
    ///
    /// # Errors
    ///
    /// A missing layer, and for `pixels` a locked layer or a group
    /// (`LayerLocked`, AC-16), before anything changes.
    pub fn begin_transform(
        &mut self,
        layer: usize,
        pixels: bool,
        settings: TransformSettings,
        gesture: Option<u64>,
    ) -> Result<bool, DocumentError> {
        let len = self.document.layers.len();
        if layer >= len {
            return Err(DocumentError::LayerIndexOutOfBounds { index: layer, len });
        }
        if pixels && super::spec::layer_locked(&self.document, layer) {
            return Err(DocumentError::LayerLocked { index: layer });
        }
        self.commit_if_live()?;
        let Some(mask) = self.document.selection.clone() else {
            return Ok(false);
        };
        let Some(bounds) = mask.bounding_box() else {
            return Ok(false);
        };
        let (original, hole, patch) = if pixels {
            let src = self.document.layers[layer].pixels.clone();
            let (patch, hole) = lift(&src, &mask, bounds);
            (Some(src), Some(hole), Some(patch))
        } else {
            (None, None, None)
        };
        let base = Arc::new(Base {
            layer,
            pixels,
            bounds,
            original,
            hole,
            patch,
            mask_patch: mask_patch(&mask, bounds),
            mask,
        });
        let settings = clamp(settings);
        let rect = base.render(&mut self.document, None, Some(&settings))?;
        let step = TransformStep {
            base,
            before: None,
            after: Shown { settings, rect },
        };
        self.damage.add(step.damage());
        self.record_selection(Some(true), true);
        self.history.push(Box::new(step));
        self.live_merge = gesture.map(MergeKey::Gesture);
        Ok(true)
    }

    /// Changes the live transform's settings (AC-02 – AC-07): the layer and
    /// outline show the transform from the lifted original. Changes of one
    /// `gesture`, or consecutive quality-only changes, form one step; a
    /// change that changes nothing adds no step.
    ///
    /// # Errors
    ///
    /// `NoLiveObject` when no transform is live.
    pub fn edit_transform(
        &mut self,
        settings: TransformSettings,
        gesture: Option<u64>,
    ) -> Result<(), DocumentError> {
        let top = self
            .top_transform_step()
            .ok_or(DocumentError::NoLiveObject)?;
        let settings = clamp(settings);
        let old = top.after;
        if settings == old.settings {
            return Ok(());
        }
        let base = Arc::clone(&top.base);
        let rect = base.render(&mut self.document, old.rect, Some(&settings))?;
        let mut step = TransformStep {
            base,
            before: Some(old),
            after: Shown { settings, rect },
        };
        self.damage.add(step.damage());
        self.record_selection(Some(true), true);
        let quality_only =
            settings.same_geometry(&old.settings) && settings.pivot == old.settings.pivot;
        let key = match gesture {
            Some(id) => Some(MergeKey::Gesture(id)),
            None if quality_only => Some(MergeKey::Group(QUALITY_GROUP)),
            None => None,
        };
        if key.is_some()
            && key == self.live_merge
            && let Some(prev) = self.history.pop_undo()
            && let Some(prev) = prev.as_any().downcast_ref::<TransformStep>()
        {
            step = TransformStep::joined(prev, &step);
        }
        self.history.push(Box::new(step));
        self.live_merge = key;
        Ok(())
    }

    /// Commits the live transform (AC-09): its steps become one step, or
    /// none when pixels and outline are as before it began (AC-17).
    ///
    /// # Errors
    ///
    /// `NoLiveObject` when no transform is live.
    pub fn commit_transform(&mut self) -> Result<(), DocumentError> {
        let n = self.transform_depth();
        let top = self
            .top_transform_step()
            .ok_or(DocumentError::NoLiveObject)?;
        let base = Arc::clone(&top.base);
        let rect = top.after.rect;
        let canvas = Rect::new(
            0,
            0,
            self.document.canvas.width(),
            self.document.canvas.height(),
        );
        let pixels = match &base.original {
            Some(original) => {
                match union(Some(base.bounds), rect).and_then(|r| r.intersect(canvas)) {
                    Some(r) => {
                        let before = original.copy_region(r)?;
                        let after = self.document.layers[base.layer].pixels.copy_region(r)?;
                        (before != after).then_some((r, before, after))
                    }
                    None => None,
                }
            }
            None => None,
        };
        let mask_after = self.document.selection.clone();
        let mask_changed = mask_after.as_ref() != Some(&base.mask);
        let keep = self.history.undo.len() - n;
        self.history.truncate_undo(keep);
        if pixels.is_some() || mask_changed {
            self.history.push(Box::new(TransformCommit {
                layer: base.layer,
                pixels,
                mask_before: base.mask.clone(),
                mask_after,
                kind: TransformStep::kind_of(&base),
            }));
        }
        self.live_merge = None;
        Ok(())
    }

    /// Cancels the live transform (AC-10): pixels and outline as before it
    /// began, and its steps leave the history.
    ///
    /// # Errors
    ///
    /// `NoLiveObject` when no transform is live.
    pub fn cancel_transform(&mut self) -> Result<(), DocumentError> {
        let n = self.transform_depth();
        let top = self
            .top_transform_step()
            .ok_or(DocumentError::NoLiveObject)?;
        let (base, rect) = (Arc::clone(&top.base), top.after.rect);
        base.render(&mut self.document, rect, None)?;
        self.damage.add(Damage::rect(
            union(Some(base.bounds), rect).unwrap_or(base.bounds),
        ));
        self.record_selection(Some(true), true);
        let keep = self.history.undo.len() - n;
        self.history.truncate_undo(keep);
        self.live_merge = None;
        Ok(())
    }
}

/// Keeps the box at least 1 px wide and high (AC-03).
fn clamp(s: TransformSettings) -> TransformSettings {
    TransformSettings {
        size: (s.size.0.max(1.0), s.size.1.max(1.0)),
        ..s
    }
}
