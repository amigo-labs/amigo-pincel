//! The live text kind (docs/specs/image-live-text.md): a text that stays
//! editable until it is committed, drawn by the Text tool into its layer at
//! every change like the other live objects (`live.rs`).

use std::sync::Arc;

use super::SetPixels;
use crate::document::Document;
use crate::geometry::Point;
use crate::tools::{Text, TextAlign, TextLayout, TextStyle};

/// The font a live text is drawn with: its registry id (as `draw_text` names
/// it) and its bytes. Two fonts are equal when their ids are.
#[derive(Debug, Clone)]
pub struct LiveFont {
    /// The font id `draw_text` uses.
    pub id: u32,
    /// The TrueType / OpenType bytes.
    pub bytes: Arc<[u8]>,
}

impl PartialEq for LiveFont {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// The parameters of a live text: its content, the top-left of its box and
/// its style (AC-01, AC-09).
#[derive(Debug, Clone, PartialEq)]
pub struct LiveText {
    /// The text; `\n` separates lines.
    pub text: String,
    /// Left edge of the text box.
    pub left: f32,
    /// Top of the text box (the first line's em box).
    pub top: f32,
    /// Size, color, bold, italic, anti-alias, alignment, underline,
    /// strikethrough.
    pub style: TextStyle,
    /// The font.
    pub font: LiveFont,
}

impl LiveText {
    /// The `draw_text` the box draws: the alignment anchor is the box's left
    /// edge plus 0, ½ or 1 times its width (AC-20), and its layout.
    pub fn anchored(&self) -> Option<(Text, TextLayout)> {
        text_box(
            &self.text,
            self.left,
            self.top,
            &self.style,
            &self.font.bytes,
        )
    }

    /// The immediate draw of the text on `doc`, or `None` when it paints
    /// nothing (empty, only whitespace, off the canvas).
    pub(crate) fn draw(&self, layer: usize, doc: &Document) -> Option<SetPixels> {
        let (text, _) = self.anchored()?;
        text.render(layer, doc, &self.font.bytes)
    }
}

/// The [`Text`] whose box has its top-left at (`left`, `top`) and its
/// layout: `x` is the alignment anchor `left + fraction × box width`
/// (docs/specs/image-live-text.md AC-20). `None` when the font fails to
/// parse.
pub fn text_box(
    content: &str,
    left: f32,
    top: f32,
    style: &TextStyle,
    font: &[u8],
) -> Option<(Text, TextLayout)> {
    let width = Text::new(content, Point::new(left, top), style.clone())
        .layout_lines(font)?
        .width;
    let fraction = match style.align {
        TextAlign::Left => 0.0,
        TextAlign::Center => 0.5,
        TextAlign::Right => 1.0,
    };
    let text = Text::new(
        content,
        Point::new(left + fraction * width, top),
        style.clone(),
    );
    let layout = text.layout_lines(font)?;
    Some((text, layout))
}
