//! Dispatch of [`CommandSpec::CloneStroke`] (docs/specs/clone-stamp.md).

use super::CommandBus;
use super::spec::{CommandSpec, StrokeScratch, azimuth_samples, parse_sample, parse_shape};
use crate::error::DocumentError;
use crate::{Brush, CloneStamp, Color, PenSamples, Point, PressureDynamics};

/// Applies a Clone Stamp stroke as the editor does: the commands sharing a
/// `stroke_id` read the source image the first of them saw and merge into
/// one undo step. Any other command is left alone.
pub(super) fn run(
    bus: &mut CommandBus,
    spec: CommandSpec,
    scratch: &mut StrokeScratch,
) -> Result<(), DocumentError> {
    let CommandSpec::CloneStroke {
        layer,
        size,
        opacity,
        shape,
        hardness,
        offset,
        sample,
        points,
        pressure,
        pressure_size,
        pressure_opacity,
        azimuth,
        angle,
        tilt_angle,
        anti_alias,
        stroke_id,
    } = spec
    else {
        return Ok(());
    };
    let brush = Brush::new(size, Color::TRANSPARENT, opacity)
        .with_shape(parse_shape(&shape))
        .with_hardness(hardness)
        .with_dynamics(PressureDynamics {
            size: pressure_size,
            opacity: pressure_opacity,
        })
        .with_angle(angle)
        .with_follow_tilt(tilt_angle)
        .with_anti_alias(anti_alias);
    let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
    let azimuth = azimuth_samples(&azimuth);
    let pen = PenSamples {
        pressure: &pressure,
        azimuth: &azimuth,
    };
    let tool = CloneStamp::new(brush, (offset[0], offset[1]), parse_sample(&sample));
    scratch.rewind(stroke_id);
    match tool.stroke_with_pen(layer, &pts, pen, &bus.document, &mut scratch.clone) {
        Some(cmd) => bus.apply(Box::new(cmd.with_stroke(stroke_id))),
        None => Ok(()), // no pixel changes — no undo step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::spec::run_command;
    use crate::document::Document;

    /// A 24×8 document: red left of x = 8, white elsewhere.
    fn bus() -> CommandBus {
        let mut doc = Document::new(24, 8).unwrap();
        for y in 0..8 {
            for x in 0..24 {
                let c = if x < 8 {
                    Color::rgba(255, 0, 0, 255)
                } else {
                    Color::WHITE
                };
                doc.layers[0].pixels.set_pixel(x, y, c);
            }
        }
        CommandBus::new(doc)
    }

    fn segment(points: &str, stroke: u64) -> CommandSpec {
        serde_json::from_str(&format!(
            r#"{{"type":"clone_stroke","layer":0,"size":4,"opacity":0.6,"shape":"soft_round","hardness":0.5,"offset":[-4,0],"points":{points},"stroke_id":{stroke}}}"#
        ))
        .expect("parses")
    }

    fn run(bus: &mut CommandBus, spec: CommandSpec, scratch: &mut StrokeScratch) {
        run_command(bus, spec, scratch, &[]).expect("runs");
    }

    #[test]
    fn clone_stroke_segments_merge_into_one_undo_step() {
        // AC-12: many segments, one step; undo restores byte for byte,
        // redo restores the stroke's result.
        let mut bus = bus();
        let before = bus.document.layers[0].pixels.clone();
        let mut scratch = StrokeScratch::default();
        for points in [
            "[[12.5,4.0]]",
            "[[12.5,4.0],[15.5,4.0]]",
            "[[15.5,4.0],[19.5,4.0]]",
        ] {
            run(&mut bus, segment(points, 7), &mut scratch);
        }
        // AC-18: the step is named after the tool, not "Draw Pixels".
        assert_eq!(
            bus.history.undo_kinds().last().map(|k| k.id().into_owned()),
            Some("clone_stamp".to_owned())
        );
        assert_eq!(bus.history.undo_depth(), 1);
        let after = bus.document.layers[0].pixels.clone();
        assert_ne!(after, before);
        assert!(bus.undo().unwrap());
        assert_eq!(bus.document.layers[0].pixels, before);
        assert!(bus.redo().unwrap());
        assert_eq!(bus.document.layers[0].pixels, after);
        // A new stroke is a step of its own.
        run(
            &mut bus,
            segment("[[13.5,2.0],[18.5,2.0]]", 8),
            &mut scratch,
        );
        assert_eq!(bus.history.undo_depth(), 2);
    }

    #[test]
    fn clone_stroke_split_across_commands_equals_one_command() {
        // AC-08 through the command format: the scratch keeps the press
        // state across the commands of one stroke (the second segment
        // reads pixels the first one wrote).
        let (mut whole, mut split) = (bus(), bus());
        let mut scratch = StrokeScratch::default();
        run(
            &mut whole,
            segment("[[12.5,4.0],[15.5,4.0],[19.5,4.0]]", 1),
            &mut scratch,
        );
        run(
            &mut split,
            segment("[[12.5,4.0],[15.5,4.0]]", 2),
            &mut scratch,
        );
        run(
            &mut split,
            segment("[[15.5,4.0],[19.5,4.0]]", 2),
            &mut scratch,
        );
        assert_eq!(
            whole.document.layers[0].pixels,
            split.document.layers[0].pixels
        );
    }

    #[test]
    fn clone_stroke_with_a_new_id_reads_the_canvas_afresh() {
        // Stroke 1 copies red to x 10..=14; stroke 2 reads there. Reusing
        // stroke 1's snapshot would copy the white it saw instead.
        let stroke = |x: f32, id: u64| -> CommandSpec {
            serde_json::from_str(&format!(
                r#"{{"type":"clone_stroke","layer":0,"size":3,"opacity":1,"offset":[-8,0],"points":[[{x},4.5],[{},4.5]],"stroke_id":{id}}}"#,
                x + 2.0
            ))
            .expect("parses")
        };
        let mut bus = bus();
        let mut scratch = StrokeScratch::default();
        run(&mut bus, stroke(10.5, 1), &mut scratch);
        run(&mut bus, stroke(18.5, 2), &mut scratch);
        assert_eq!(
            bus.document.layers[0].pixels.get_pixel(19, 4),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn clone_stroke_defaults_to_the_current_layer() {
        // The other layer is transparent: with the default sample nothing
        // is copied into it; `all_layers` copies the red below.
        let spec = |sample: &str| -> CommandSpec {
            serde_json::from_str(&format!(
                r#"{{"type":"clone_stroke","layer":1,"size":3,"opacity":1,"offset":[-12,0],{sample}"points":[[14.5,4.5]],"stroke_id":1}}"#
            ))
            .expect("parses")
        };
        let mut bus = bus();
        bus.document.add_layer("Top").unwrap();
        run(&mut bus, spec(""), &mut StrokeScratch::default());
        assert_eq!(bus.history.undo_depth(), 0);
        run(
            &mut bus,
            spec(r#""sample":"all_layers","#),
            &mut StrokeScratch::default(),
        );
        assert_eq!(
            bus.document.layers[1].pixels.get_pixel(14, 4),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn clone_stroke_on_a_locked_layer_is_refused() {
        // AC-14 / AC-20: the editor's and the agent's lock check.
        let mut bus = bus();
        bus.document.layers[0].locked = true;
        assert!(segment("[[12.5,4.0]]", 1).refused(&bus.document));
    }
}
