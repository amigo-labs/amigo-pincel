//! The font registry: the faces `register_font` hands out ids for. A face is
//! a font file plus the index of the face inside it (a collection holds
//! several, docs/specs/system-fonts.md AC-08). Each file is kept once, however
//! many of its faces are registered; ids live as long as the module.

use std::cell::RefCell;

use pincel_image_core::FontFace;

#[derive(Default)]
struct Registry {
    /// Font files, each byte blob once.
    files: Vec<Vec<u8>>,
    /// Registered faces by id: index into `files`, face index.
    faces: Vec<(usize, u32)>,
}

thread_local! {
    /// The registered faces. The Text tool re-parses these per commit
    /// (ADR-012), so JS uploads each face once and references it by id rather
    /// than passing bytes per command.
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// Registers face `index` of `data` and returns its id; registering the same
/// bytes and index again returns the existing id.
pub(crate) fn register(data: &[u8], index: u32) -> u32 {
    REGISTRY.with(|r| {
        let mut r = r.borrow_mut();
        let file = match r.files.iter().position(|f| f == data) {
            Some(file) => file,
            None => {
                r.files.push(data.to_vec());
                r.files.len() - 1
            }
        };
        if let Some(id) = r.faces.iter().position(|&f| f == (file, index)) {
            return id as u32;
        }
        r.faces.push((file, index));
        (r.faces.len() - 1) as u32
    })
}

/// Runs `f` with every registered face, indexed by id.
pub(crate) fn with_faces<T>(f: impl FnOnce(&[FontFace<'_>]) -> T) -> T {
    REGISTRY.with(|r| {
        let r = r.borrow();
        let faces: Vec<FontFace<'_>> = r
            .faces
            .iter()
            .map(|&(file, index)| FontFace::new(&r.files[file], index))
            .collect();
        f(&faces)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTC: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pincel-core/tests/fixtures/PincelTestSerif.ttc"
    ));

    #[test]
    fn register_same_bytes_and_index_returns_the_same_id() {
        let a = register(TTC, 0);
        assert_eq!(register(TTC, 0), a);
        let b = register(TTC, 1);
        assert_ne!(a, b, "another face of the collection");
        assert_eq!(register(TTC, 1), b);
    }

    #[test]
    fn with_faces_names_each_registered_face_by_id() {
        let a = register(TTC, 1);
        with_faces(|faces| {
            assert_eq!(faces[a as usize].index, 1);
            assert_eq!(faces[a as usize].bytes, TTC);
        });
        REGISTRY.with(|r| {
            let r = r.borrow();
            assert_eq!(r.files.iter().filter(|f| f.as_slice() == TTC).count(), 1);
        });
    }
}
