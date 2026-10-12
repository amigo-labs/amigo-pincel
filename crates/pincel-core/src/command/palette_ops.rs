//! Palette editing commands (spec §3.7): recolour, add, remove and
//! reorder entries, and replace the whole palette (presets, `.gpl` import).
//!
//! In RGBA documents the palette is a swatch collection — no pixel
//! references an entry, so these are metadata edits with
//! [`DirtyRegion::None`]. In indexed documents pixels *are* entry
//! indices: recolouring is still well-defined (every pixel using the
//! entry changes colour), but removing or reordering entries would
//! silently repaint the image, so those two are refused with
//! [`CommandError::UnsupportedColorMode`] until indexed painting lands.

use crate::document::{
    CelMap, ColorMode, MAX_PALETTE_ENTRIES, Palette, PaletteEntry, Rgba, Sprite,
};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the colour of palette entry `index`. Consecutive edits to the same
/// entry merge (a colour-picker drag is one undo step).
#[derive(Debug, Clone)]
pub struct SetPaletteColor {
    index: usize,
    rgba: Rgba,
    prev: Option<Rgba>,
}

impl SetPaletteColor {
    pub fn new(index: usize, rgba: Rgba) -> Self {
        Self {
            index,
            rgba,
            prev: None,
        }
    }
}

impl Command for SetPaletteColor {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let entry = doc
            .palette
            .colors
            .get_mut(self.index)
            .ok_or(CommandError::UnknownPaletteEntry(self.index))?;
        self.prev = Some(entry.rgba);
        entry.rgba = self.rgba;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(prev) = self.prev.take()
            && let Some(entry) = doc.palette.colors.get_mut(self.index)
        {
            entry.rgba = prev;
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if self.index != next.index {
            return false;
        }
        self.rgba = next.rgba;
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Append an entry to the palette.
#[derive(Debug, Clone)]
pub struct AddPaletteEntry {
    entry: PaletteEntry,
    applied: bool,
}

impl AddPaletteEntry {
    pub fn new(entry: PaletteEntry) -> Self {
        Self {
            entry,
            applied: false,
        }
    }
}

impl Command for AddPaletteEntry {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if doc.palette.colors.len() >= MAX_PALETTE_ENTRIES {
            return Err(CommandError::PaletteFull);
        }
        doc.palette.colors.push(self.entry.clone());
        self.applied = true;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if std::mem::take(&mut self.applied) {
            doc.palette.colors.pop();
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

fn require_rgba(doc: &Sprite) -> Result<(), CommandError> {
    if doc.color_mode == ColorMode::Rgba {
        Ok(())
    } else {
        Err(CommandError::UnsupportedColorMode)
    }
}

/// Remove palette entry `index` (RGBA documents only).
#[derive(Debug, Clone)]
pub struct RemovePaletteEntry {
    index: usize,
    removed: Option<PaletteEntry>,
}

impl RemovePaletteEntry {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            removed: None,
        }
    }
}

impl Command for RemovePaletteEntry {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        require_rgba(doc)?;
        if self.index >= doc.palette.colors.len() {
            return Err(CommandError::UnknownPaletteEntry(self.index));
        }
        self.removed = Some(doc.palette.colors.remove(self.index));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(entry) = self.removed.take() {
            let at = self.index.min(doc.palette.colors.len());
            doc.palette.colors.insert(at, entry);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Move palette entry `from` to position `to` (RGBA documents only); the
/// entries in between shift by one.
#[derive(Debug, Clone)]
pub struct MovePaletteEntry {
    from: usize,
    to: usize,
    applied: bool,
}

impl MovePaletteEntry {
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            applied: false,
        }
    }
}

fn move_entry(colors: &mut Vec<PaletteEntry>, from: usize, to: usize) {
    let entry = colors.remove(from);
    colors.insert(to, entry);
}

impl Command for MovePaletteEntry {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        require_rgba(doc)?;
        let len = doc.palette.colors.len();
        for i in [self.from, self.to] {
            if i >= len {
                return Err(CommandError::UnknownPaletteEntry(i));
            }
        }
        move_entry(&mut doc.palette.colors, self.from, self.to);
        self.applied = true;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if std::mem::take(&mut self.applied) {
            move_entry(&mut doc.palette.colors, self.to, self.from);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Replace the whole palette (a preset or an imported `.gpl`) in one undo
/// step. RGBA documents only, like remove / reorder: swapping the palette
/// under an indexed image would repaint it.
#[derive(Debug, Clone)]
pub struct ReplacePalette {
    palette: Palette,
    prev: Option<Palette>,
}

impl ReplacePalette {
    pub fn new(palette: Palette) -> Self {
        Self {
            palette,
            prev: None,
        }
    }
}

impl Command for ReplacePalette {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        require_rgba(doc)?;
        if self.palette.len() > MAX_PALETTE_ENTRIES {
            return Err(CommandError::PaletteFull);
        }
        self.prev = Some(std::mem::replace(&mut doc.palette, self.palette.clone()));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(prev) = self.prev.take() {
            doc.palette = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Frame, Layer, LayerId, Palette};

    fn doc() -> (Sprite, CelMap) {
        let mut sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        sprite.palette = Palette::from_entries(vec![
            PaletteEntry::new(Rgba::BLACK),
            PaletteEntry::new(Rgba::WHITE),
            PaletteEntry::with_name(Rgba::new(255, 0, 0, 255), "red"),
        ]);
        (sprite, CelMap::new())
    }

    fn colors(s: &Sprite) -> Vec<Rgba> {
        s.palette.colors.iter().map(|e| e.rgba).collect()
    }

    #[test]
    fn set_palette_color_applies_reverts_and_merges() {
        let (mut s, mut c) = doc();
        let mut first = SetPaletteColor::new(0, Rgba::new(1, 2, 3, 255));
        first.apply(&mut s, &mut c).expect("apply");
        let mut second = SetPaletteColor::new(0, Rgba::new(4, 5, 6, 255));
        second.apply(&mut s, &mut c).expect("apply");
        assert!(first.merge(&second));
        assert_eq!(s.palette.colors[0].rgba, Rgba::new(4, 5, 6, 255));
        first.revert(&mut s, &mut c);
        assert_eq!(s.palette.colors[0].rgba, Rgba::BLACK);
    }

    #[test]
    fn set_palette_color_unknown_index_returns_err() {
        let (mut s, mut c) = doc();
        assert_eq!(
            SetPaletteColor::new(9, Rgba::BLACK).apply(&mut s, &mut c),
            Err(CommandError::UnknownPaletteEntry(9))
        );
    }

    #[test]
    fn add_palette_entry_appends_and_reverts() {
        let (mut s, mut c) = doc();
        let mut cmd = AddPaletteEntry::new(PaletteEntry::new(Rgba::TRANSPARENT));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.palette.len(), 4);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.palette.len(), 3);
    }

    #[test]
    fn add_palette_entry_to_full_palette_returns_err() {
        let (mut s, mut c) = doc();
        s.palette.colors = vec![PaletteEntry::new(Rgba::BLACK); MAX_PALETTE_ENTRIES];
        assert_eq!(
            AddPaletteEntry::new(PaletteEntry::new(Rgba::WHITE)).apply(&mut s, &mut c),
            Err(CommandError::PaletteFull)
        );
    }

    #[test]
    fn remove_palette_entry_round_trips_with_name() {
        let (mut s, mut c) = doc();
        let before = s.palette.clone();
        let mut cmd = RemovePaletteEntry::new(2);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(colors(&s), vec![Rgba::BLACK, Rgba::WHITE]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.palette, before);
    }

    #[test]
    fn move_palette_entry_shifts_between_and_reverts() {
        let (mut s, mut c) = doc();
        let before = s.palette.clone();
        let mut cmd = MovePaletteEntry::new(0, 2);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(
            colors(&s),
            vec![Rgba::WHITE, Rgba::new(255, 0, 0, 255), Rgba::BLACK]
        );
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.palette, before);
    }

    #[test]
    fn remove_and_move_refuse_indexed_documents() {
        let (mut s, mut c) = doc();
        s.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        assert_eq!(
            RemovePaletteEntry::new(0).apply(&mut s, &mut c),
            Err(CommandError::UnsupportedColorMode)
        );
        assert_eq!(
            MovePaletteEntry::new(0, 1).apply(&mut s, &mut c),
            Err(CommandError::UnsupportedColorMode)
        );
    }

    #[test]
    fn replace_palette_swaps_and_reverts() {
        let mut sprite = Sprite::builder(2, 2)
            .palette(Palette::db32())
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        let next = Palette::preset(crate::document::PalettePreset::GameBoy);
        let mut cmd = ReplacePalette::new(next.clone());
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.palette, next);
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.palette.len(), 32);
    }

    #[test]
    fn replace_palette_refused_on_indexed_sprites() {
        let mut sprite = Sprite::builder(2, 2).build().expect("sprite builds");
        sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let err = ReplacePalette::new(Palette::db32())
            .apply(&mut sprite, &mut CelMap::new())
            .unwrap_err();
        assert!(matches!(err, CommandError::UnsupportedColorMode));
    }
}
