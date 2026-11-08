//! `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` on Pixel
//! documents: the Pixel editor's Pencil / Eraser (with the Shading ink,
//! symmetry and tiled wrapping), shape tools, Text tool and dithered
//! Gradient. Each runs the editor's core command on a scratch copy of the
//! target cel and commits the difference as one undo step. See
//! docs/specs/mcp-parity-drawing.md.

use pincel_core::{
    BrushShape, Cel, CelMap, ColorMode, Command, DitherMatrix, DrawDitherGradient, DrawShape,
    DrawText, GradientKind, HistoryKind, Ink, Mirror, PencilStroke, PixelBrush, Point, Rgba,
    ShapeKind, ShapeMode, ShapeStyle, Sprite, TextAlign, TextStyle, Wrap,
};
use serde_json::Value;

use super::draw::{paint, run_core};
use crate::args::{Args, parse_color};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, ToolOutput};

/// Refuse arguments only the Image editor offers (AC-18).
fn refuse_image_args(id: &str, args: &Args, keys: &[&str]) -> Result<(), AgentError> {
    match keys.iter().find(|k| args.get(k).is_some()) {
        Some(k) => Err(AgentError::tool(format!(
            "argument `{k}` works on Image documents only; document `{id}` is a Pixel document"
        ))),
        None => Ok(()),
    }
}

/// An integer argument within `lo..=hi` on Pixel documents (AC-24).
fn ranged(args: &Args, key: &str, lo: i64, hi: i64, default: i64) -> Result<i64, AgentError> {
    let Some(v) = args.get(key) else {
        return Ok(default);
    };
    match v.as_f64() {
        Some(n) if n.fract() == 0.0 && (lo as f64..=hi as f64).contains(&n) => Ok(n as i64),
        _ => Err(AgentError::tool(format!(
            "`{key}` must be {lo}–{hi} on Pixel documents"
        ))),
    }
}

/// An integer coordinate (AC-09): Pixel tools take whole pixels.
fn int_coord(args: &Args, key: &str) -> Result<i32, AgentError> {
    let v = args
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))?;
    if v.fract() != 0.0 || v < f64::from(i32::MIN) || v > f64::from(i32::MAX) {
        return Err(AgentError::tool(format!(
            "argument `{key}`: Pixel documents take integer coordinates (got {v})"
        )));
    }
    Ok(v as i32)
}

fn color_or_black(doc: &Doc, args: &Args, key: &str) -> Result<Rgba, AgentError> {
    match args.get(key) {
        Some(v) => parse_color(v, key, &doc.sprite.palette),
        None => Ok(Rgba::new(0, 0, 0, 255)),
    }
}

/// The integer stroke points (AC-02, AC-09).
fn stroke_points(args: &Args) -> Result<Vec<Point>, AgentError> {
    let items = args
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::tool("missing required argument `points`"))?;
    if items.is_empty() {
        return Err(AgentError::tool("`points` needs at least one [x, y] point"));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let xy = p.as_array().filter(|a| a.len() == 2);
            let get = |k: usize| xy.and_then(|a| a[k].as_f64());
            match (get(0), get(1)) {
                (Some(x), Some(y)) if x.fract() == 0.0 && y.fract() == 0.0 => {
                    Ok(Point::new(x as i32, y as i32))
                }
                (Some(_), Some(_)) => Err(AgentError::tool(format!(
                    "points[{i}]: Pixel documents take integer coordinates (got {p})"
                ))),
                _ => Err(AgentError::tool(format!(
                    "points[{i}] must be an [x, y] pair of numbers"
                ))),
            }
        })
        .collect()
}

fn axes(name: Option<&str>, key: &str) -> Result<(bool, bool), AgentError> {
    match name {
        None => Ok((false, false)),
        Some("x") => Ok((true, false)),
        Some("y") => Ok((false, true)),
        Some("xy") => Ok((true, true)),
        Some(other) => Err(AgentError::tool(format!(
            "argument `{key}`: unknown value {other:?} (x, y, xy)"
        ))),
    }
}

/// The symmetry axes (AC-07, AC-09).
fn mirror_arg(doc: &Doc, args: &Args) -> Result<Option<Mirror>, AgentError> {
    let (mx, my) = axes(args.str("mirror"), "mirror")?;
    let axis = |key: &str, on: bool, side: u32| -> Result<Option<i32>, AgentError> {
        let given = args.get(key);
        if given.is_some() && !on {
            return Err(AgentError::tool(format!(
                "argument `{key}` needs `mirror` to cover that axis"
            )));
        }
        if !on {
            return Ok(None);
        }
        let max = i64::from(side) * 2;
        match given {
            None => Ok(Some(side as i32)),
            Some(v) => match v.as_i64() {
                Some(n) if (0..=max).contains(&n) => Ok(Some(n as i32)),
                _ => Err(AgentError::tool(format!(
                    "argument `{key}` must be 0–{max} (half pixels from the canvas edge)"
                ))),
            },
        }
    };
    let axis_x2 = axis("axis_x2", mx, doc.sprite.width)?;
    let axis_y2 = axis("axis_y2", my, doc.sprite.height)?;
    Ok((mx || my).then_some(Mirror { axis_x2, axis_y2 }))
}

/// The Shading ink (AC-05, AC-06).
fn shading_ink(doc: &Doc, args: &Args) -> Result<Ink, AgentError> {
    let palette = &doc.sprite.palette;
    if palette.is_empty() {
        return Err(AgentError::tool(
            "the shading ink needs a palette, and the palette is empty",
        ));
    }
    let len = palette.len() as u64;
    let index = |key: &str| -> Result<Option<usize>, AgentError> {
        match args.get(key) {
            None => Ok(None),
            Some(v) => match v.as_u64() {
                Some(i) if i < len => Ok(Some(i as usize)),
                _ => Err(AgentError::tool(format!(
                    "argument `{key}`: palette index {v} is not in the palette ({len} entries)"
                ))),
            },
        }
    };
    let (from, to) = match (index("ramp_from")?, index("ramp_to")?) {
        (None, None) => (0, palette.len() - 1),
        (Some(f), Some(t)) if f <= t => (f, t),
        (Some(f), Some(t)) => {
            return Err(AgentError::tool(format!(
                "argument `ramp_from` ({f}) is greater than `ramp_to` ({t})"
            )));
        }
        (Some(_), None) => return Err(AgentError::tool("argument `ramp_from` needs `ramp_to`")),
        (None, Some(_)) => return Err(AgentError::tool("argument `ramp_to` needs `ramp_from`")),
    };
    let step = match args.get("step").map(Value::as_i64) {
        None | Some(Some(1)) => 1,
        Some(Some(-1)) => -1,
        Some(_) => return Err(AgentError::tool("argument `step` must be 1 or -1")),
    };
    let ramp = palette.colors[from..=to].iter().map(|e| e.rgba).collect();
    Ok(Ink::Shading { ramp, step })
}

/// Run a Pencil stroke through `points` on `cel` alone.
fn run_stroke(
    sprite: &Sprite,
    cel: &mut Cel,
    mut cmd: PencilStroke,
    rest: &[Point],
) -> Result<(), AgentError> {
    let mut scratch = sprite.clone();
    let mut cels = CelMap::new();
    let key = (cel.layer, cel.frame);
    cels.insert(cel.clone());
    let fail = |e: pincel_core::CommandError| AgentError::tool(e.to_string());
    cmd.apply(&mut scratch, &mut cels).map_err(fail)?;
    for &p in rest {
        cmd.extend_to(p, 1.0, &mut cels).map_err(fail)?;
    }
    *cel = cels
        .remove(key.0, key.1)
        .ok_or_else(|| AgentError::tool("the target cel vanished during the edit"))?;
    Ok(())
}

/// `draw.stroke` on a Pixel document.
pub(crate) fn stroke(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    {
        let (id, doc) = ctx.ws.doc(args)?;
        refuse_image_args(
            &id,
            args,
            &[
                "opacity",
                "hardness",
                "eraser_mode",
                "background",
                "angle",
                "anti_alias",
            ],
        )?;
        if let Some(s @ ("hard_round" | "soft_round" | "flat")) = args.str("shape") {
            return Err(AgentError::tool(format!(
                "argument `shape`: {s:?} works on Image documents only; document `{id}` is a Pixel document (square, round)"
            )));
        }
        let _ = doc;
    }
    let points = stroke_points(args)?;
    let size = ranged(args, "size", 1, 8, 1)? as u8;
    let shape = match args.str("shape").unwrap_or("square") {
        "square" => BrushShape::Square,
        _ => BrushShape::Round,
    };
    let eraser = args.str("tool") == Some("eraser");
    let shading = match args.str("ink").unwrap_or("simple") {
        "simple" => false,
        "shading" => true,
        other => {
            return Err(AgentError::tool(format!(
                "argument `ink`: unknown ink {other:?} (simple, shading)"
            )));
        }
    };
    if shading && eraser {
        return Err(AgentError::tool(
            "argument `tool`: the eraser has no shading ink; use `tool: \"brush\"` with `ink: \"shading\"`",
        ));
    }
    let pixel_perfect = args.bool("pixel_perfect").unwrap_or(false);
    let (wx, wy) = axes(args.str("wrap"), "wrap")?;
    let (ink, color, mirror) = {
        let (_, doc) = ctx.ws.doc(args)?;
        let ink = if shading {
            shading_ink(doc, args)?
        } else {
            Ink::Simple
        };
        let color = if eraser || shading {
            Rgba::TRANSPARENT
        } else {
            color_or_black(doc, args, "color")?
        };
        (ink, color, mirror_arg(doc, args)?)
    };
    let history_kind = match (eraser, shading) {
        (true, _) => HistoryKind::Eraser,
        (false, true) => HistoryKind::Shading,
        (false, false) => HistoryKind::Pencil,
    };
    paint(ctx, args, history_kind, |doc, cel| {
        let mut cmd = PencilStroke::new(
            cel.layer,
            cel.frame,
            color,
            PixelBrush::new(size, shape),
            pixel_perfect,
            points[0],
            1.0,
        )
        .with_ink(ink.clone())
        .with_wrap(Wrap { x: wx, y: wy });
        if let Some(m) = mirror {
            cmd = cmd.with_mirror(m);
        }
        run_stroke(&doc.sprite, cel, cmd, &points[1..])
    })
}

/// `draw.shape` on a Pixel document.
pub(crate) fn shape(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let style_colors = {
        let (id, doc) = ctx.ws.doc(args)?;
        if args.str("shape") == Some("line") {
            return Err(AgentError::tool(format!(
                "argument `shape`: \"line\" works on Image documents only; document `{id}` is a Pixel document, use draw.line"
            )));
        }
        for key in ["cx", "cy", "radius"] {
            if args.get(key).is_some() {
                return Err(AgentError::tool(format!(
                    "argument `{key}` works on Image documents only; document `{id}` is a Pixel document: give the polygon's box by the corners (x0, y0)–(x1, y1)"
                )));
            }
        }
        refuse_image_args(&id, args, &["rotation", "dash", "anti_alias"])?;
        let stroke = args
            .get("stroke")
            .map(|v| parse_color(v, "stroke", &doc.sprite.palette))
            .transpose()?;
        let fill = args
            .get("fill")
            .map(|v| parse_color(v, "fill", &doc.sprite.palette))
            .transpose()?;
        (stroke, fill)
    };
    let (stroke, fill) = style_colors;
    let mode = match (stroke, fill) {
        (Some(_), Some(_)) => ShapeMode::FillOutline,
        (None, Some(_)) => ShapeMode::Fill,
        (Some(_), None) => ShapeMode::Outline,
        (None, None) => {
            return Err(AgentError::tool(
                "draw.shape needs a `stroke` color, a `fill` color or both",
            ));
        }
    };
    let (x0, y0) = (int_coord(args, "x0")?, int_coord(args, "y0")?);
    let (x1, y1) = (int_coord(args, "x1")?, int_coord(args, "y1")?);
    let stroke_width = ranged(args, "stroke_width", 1, 64, 1)? as u32;
    let kind = match args.req_str("shape")? {
        "rect" => ShapeKind::Rectangle,
        "rounded" => ShapeKind::RoundedRectangle {
            radius: ranged(args, "corner_radius", 0, 256, 4)? as f64,
        },
        "ellipse" => ShapeKind::Ellipse,
        "polygon" => ShapeKind::Polygon {
            sides: ranged(args, "sides", 3, 64, 6)? as u32,
            rotation_deg: 0.0,
        },
        other => {
            return Err(AgentError::tool(format!(
                "argument `shape`: unknown shape {other:?}"
            )));
        }
    };
    let style = ShapeStyle {
        mode,
        stroke_width,
        stroke: stroke.unwrap_or(Rgba::TRANSPARENT),
        fill: fill.unwrap_or(Rgba::TRANSPARENT),
    };
    let history_kind = match kind {
        ShapeKind::Rectangle => HistoryKind::Rectangle,
        ShapeKind::RoundedRectangle { .. } => HistoryKind::RoundedRectangle,
        ShapeKind::Ellipse => HistoryKind::Ellipse,
        ShapeKind::Polygon { .. } => HistoryKind::Polygon,
    };
    paint(ctx, args, history_kind, |doc, cel| {
        let cmd = DrawShape::new(cel.layer, cel.frame, (x0, y0), (x1, y1), kind, style);
        run_core(&doc.sprite, cel, cmd)
    })
}

/// `draw.gradient` on a Pixel document.
pub(crate) fn gradient(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let colors = {
        let (id, doc) = ctx.ws.doc(args)?;
        refuse_image_args(&id, args, &["opacity", "channel"])?;
        if let Some(k @ ("reflected" | "conical" | "diamond" | "spiral_cw" | "spiral_ccw")) =
            args.str("kind")
        {
            return Err(AgentError::tool(format!(
                "argument `kind`: {k:?} works on Image documents only; document `{id}` is a Pixel document (linear, radial)"
            )));
        }
        if doc.sprite.color_mode != ColorMode::Rgba {
            return Err(AgentError::tool(
                "the gradient paints RGBA documents only; this document is indexed or grayscale",
            ));
        }
        let from = parse_color(
            args.get("from").unwrap_or(&Value::Null),
            "from",
            &doc.sprite.palette,
        )?;
        let to = parse_color(
            args.get("to").unwrap_or(&Value::Null),
            "to",
            &doc.sprite.palette,
        )?;
        (from, to)
    };
    let (x0, y0) = (int_coord(args, "x0")?, int_coord(args, "y0")?);
    let (x1, y1) = (int_coord(args, "x1")?, int_coord(args, "y1")?);
    let kind = GradientKind::from_name(args.str("kind").unwrap_or("linear"))
        .ok_or_else(|| AgentError::tool("argument `kind`: unknown gradient kind"))?;
    let dither = match args.get("dither").map(Value::as_u64) {
        None => DitherMatrix::from_size(4),
        Some(Some(n @ (0 | 2 | 4 | 8))) => DitherMatrix::from_size(n as u32),
        Some(_) => None,
    }
    .ok_or_else(|| AgentError::tool("`dither` must be 0, 2, 4 or 8 on Pixel documents"))?;
    paint(ctx, args, HistoryKind::Gradient, |doc, cel| {
        match DrawDitherGradient::new(
            cel.layer,
            cel.frame,
            (x0, y0),
            (x1, y1),
            kind,
            dither,
            colors,
        ) {
            Some(cmd) => run_core(&doc.sprite, cel, cmd),
            // A zero-length drag paints nothing, as in the editor.
            None => Ok(()),
        }
    })
}

/// `draw.text` on a Pixel document.
pub(crate) fn text(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let color = {
        let (id, doc) = ctx.ws.doc(args)?;
        // The Pixel editor's Text dialog has no underline or strikethrough.
        refuse_image_args(&id, args, &["underline", "strikethrough"])?;
        color_or_black(doc, args, "color")?
    };
    let (x, y) = (int_coord(args, "x")?, int_coord(args, "y")?);
    let size = match args.get("size") {
        None => 12.0,
        Some(v) => match v.as_f64() {
            Some(n) if (1.0..=2000.0).contains(&n) => n as f32,
            _ => return Err(AgentError::tool("`size` must be 1–2000 on Pixel documents")),
        },
    };
    let align = TextAlign::from_name(args.str("align").unwrap_or("left"))
        .ok_or_else(|| AgentError::tool("argument `align`: unknown alignment"))?;
    let style = TextStyle {
        size,
        color,
        bold: args.bool("bold").unwrap_or(false),
        italic: args.bool("italic").unwrap_or(false),
        anti_alias: args.bool("anti_alias").unwrap_or(false),
        align,
    };
    let text = args.req_str("text")?.to_owned();
    paint(ctx, args, HistoryKind::Text, |doc, cel| {
        let cmd = DrawText::new(
            cel.layer,
            cel.frame,
            (x, y),
            text.clone(),
            crate::image::FONT.to_vec(),
            style,
        );
        run_core(&doc.sprite, cel, cmd)
    })
}
