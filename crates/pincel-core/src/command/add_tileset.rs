//! `AddTileset` command — append a tileset to the sprite's tileset list.
//!
//! Tileset z-order is irrelevant (lookup is by id), so this is an append-
//! only command. Use `Sprite::tilesets` reordering at the sprite level if
//! reordering is ever needed.

use crate::document::{CelMap, Sprite, Tileset};

use super::Command;
use super::error::CommandError;

/// Append a [`Tileset`] to the sprite's tileset list. `revert` pops it back.
#[derive(Debug, Clone)]
pub struct AddTileset {
    /// `Some` until applied; consumed on `apply` and re-populated on `revert`.
    tileset: Option<Tileset>,
    /// `Some(index)` after a successful `apply`; consumed on `revert`.
    inserted_index: Option<usize>,
}

impl AddTileset {
    /// Append `tileset` to the sprite's tileset list.
    pub fn new(tileset: Tileset) -> Self {
        Self {
            tileset: Some(tileset),
            inserted_index: None,
        }
    }
}

impl Command for AddTileset {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let tileset = self
            .tileset
            .as_ref()
            .expect("AddTileset applied without a tileset payload");

        if doc.tilesets.iter().any(|t| t.id == tileset.id) {
            return Err(CommandError::DuplicateTilesetId(tileset.id.0));
        }

        let tileset = self.tileset.take().expect("payload checked above");
        let index = doc.tilesets.len();
        doc.tilesets.push(tileset);
        self.inserted_index = Some(index);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(index) = self.inserted_index.take() else {
            return;
        };
        if index < doc.tilesets.len() {
            let tileset = doc.tilesets.remove(index);
            self.tileset = Some(tileset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::TilesetId;

    fn empty_doc() -> (Sprite, CelMap) {
        (
            Sprite::builder(8, 8).build().expect("sprite builds"),
            CelMap::new(),
        )
    }

    #[test]
    fn apply_appends_tileset() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddTileset::new(Tileset::new(TilesetId::new(1), "ground", (16, 16)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.tilesets.len(), 1);
        assert_eq!(sprite.tilesets[0].id, TilesetId::new(1));
    }

    #[test]
    fn revert_removes_inserted_tileset() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddTileset::new(Tileset::new(TilesetId::new(7), "t", (8, 8)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.tilesets.len(), 1);
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.tilesets.is_empty());
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let (mut sprite, mut cels) = empty_doc();
        AddTileset::new(Tileset::new(TilesetId::new(1), "a", (8, 8)))
            .apply(&mut sprite, &mut cels)
            .expect("apply 1");
        let mut dup = AddTileset::new(Tileset::new(TilesetId::new(1), "b", (8, 8)));
        assert_eq!(
            dup.apply(&mut sprite, &mut cels),
            Err(CommandError::DuplicateTilesetId(1))
        );
        assert_eq!(sprite.tilesets.len(), 1);
    }

    #[test]
    fn apply_revert_apply_round_trip_restores_state() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddTileset::new(Tileset::new(TilesetId::new(2), "t", (8, 8)));
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.tilesets.is_empty());
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.tilesets.len(), 1);
        assert_eq!(sprite.tilesets[0].id, TilesetId::new(2));
    }
}
