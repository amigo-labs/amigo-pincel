# aseprite-writer

Standalone Rust crate that writes [Aseprite] v1.3 (`.aseprite` / `.ase`)
files. Designed as the write-side counterpart to [`aseprite-loader`]: it
mirrors the loader's data model so a user can read with the loader,
modify, and re-emit with this crate without translating through any
intermediate types.

## Status

Phase 1 — partial. The chunks listed in
`docs/specs/pincel.md` §8.3 are being implemented incrementally. See
`STATUS.md` at the workspace root for the current milestone.

## Format reference

The binary format is described in the [Aseprite file specs][ase-spec].
This crate targets v1.3 of that document.

[ase-spec]: https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md

## License

Dual-licensed under MIT or Apache-2.0, matching `aseprite-loader`.

## Trademark

"Aseprite" is a trademark of [Igara Studio S.A.] This crate is **not**
affiliated with, endorsed by, or sponsored by Igara Studio. It only
implements the publicly-documented file format.

[Aseprite]: https://www.aseprite.org/
[`aseprite-loader`]: https://crates.io/crates/aseprite-loader
[Igara Studio S.A.]: https://igara.com/
