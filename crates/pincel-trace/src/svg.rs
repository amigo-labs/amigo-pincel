//! The SVG document of a trace result (`docs/specs/vectorize-core.md`
//! AC-14).

use crate::potrace::js_round;
use crate::session::{ResultKind, TraceResult};

/// Builds the SVG document: `viewBox`, `width` and `height` of the traced
/// image, one even-odd `path` per layer in result order. Colour-mode layers
/// carry a 0.5 px stroke in their own colour so no hairline gap shows between
/// neighbouring colours; pixel-exact documents render with crisp edges
/// instead, since their layers meet exactly on pixel edges.
pub fn to_svg(result: &TraceResult) -> String {
    let (w, h) = (result.width, result.height);
    let crisp = if result.kind == ResultKind::PixelExact {
        r#" shape-rendering="crispEdges""#
    } else {
        ""
    };
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"{crisp}>"#
    );
    for layer in &result.layers {
        let hex = layer.color.rgb().hex();
        out.push_str(&format!(r#"<path d="{}" fill="{hex}""#, layer.path_data));
        if layer.color.a < 255 {
            let opacity = js_round(f64::from(layer.color.a) / 255.0 * 1000.0) / 1000.0;
            out.push_str(&format!(r#" fill-opacity="{opacity}""#));
        }
        if result.kind == ResultKind::Color {
            out.push_str(&format!(
                r#" stroke="{hex}" stroke-width="0.5" stroke-linejoin="round""#
            ));
        }
        out.push_str(r#" fill-rule="evenodd"/>"#);
    }
    out.push_str("</svg>");
    out
}
