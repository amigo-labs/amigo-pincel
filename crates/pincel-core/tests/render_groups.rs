//! `compose()` folds group layers — feature spec
//! `docs/specs/linked-cels-group-compose.md` AC-08 … AC-20, design
//! `docs/specs/pincel.md` §4.2 ("group layers fold their children into a
//! temporary buffer") and the Decision Log entry of 2026-10-29 (a 255 /
//! Normal group passes its children through).

use pincel_core::{
    BlendMode, Cel, CelData, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex, Layer,
    LayerFilter, LayerId, OnionSkin, PixelBuffer, Rect, RenderError, Sprite, compose,
    read_aseprite, write_aseprite,
};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREY: [u8; 4] = [128, 128, 128, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.as_chunks_mut::<4>().0 {
        *px = rgba;
    }
    buf
}

fn group(id: u32, parent: Option<u32>, opacity: u8, blend_mode: BlendMode) -> Layer {
    let mut l = Layer::group(LayerId::new(id), format!("g{id}"));
    l.parent = parent.map(LayerId::new);
    l.opacity = opacity;
    l.blend_mode = blend_mode;
    l
}

fn image(id: u32, parent: Option<u32>, blend_mode: BlendMode) -> Layer {
    let mut l = Layer::image(LayerId::new(id), format!("l{id}"));
    l.parent = parent.map(LayerId::new);
    l.blend_mode = blend_mode;
    l
}

/// A 1×1, one-frame sprite of `layers` where each `(id, color)` image layer
/// is filled with `color`.
fn doc(layers: Vec<Layer>, fills: &[(u32, [u8; 4])]) -> (Sprite, CelMap) {
    let mut b = Sprite::builder(1, 1).add_frame(Frame::new(100));
    for l in layers {
        b = b.add_layer(l);
    }
    let sprite = b.build().expect("sprite builds");
    let mut cels = CelMap::new();
    for &(id, color) in fills {
        cels.insert(Cel::image(
            LayerId::new(id),
            FrameIndex::new(0),
            solid(1, 1, color),
        ));
    }
    (sprite, cels)
}

fn compose_with(sprite: &Sprite, cels: &CelMap, filter: LayerFilter) -> Vec<u8> {
    let mut out = Vec::new();
    let req = ComposeRequest {
        include_layers: filter,
        ..ComposeRequest::full(FrameIndex::new(0), sprite.width, sprite.height)
    };
    compose(sprite, cels, &req, &mut out).expect("compose succeeds");
    out
}

fn compose_visible(sprite: &Sprite, cels: &CelMap) -> Vec<u8> {
    compose_with(sprite, cels, LayerFilter::Visible)
}

#[test]
fn compose_hides_layers_inside_hidden_group_at_any_depth() {
    // AC-08: g1 > g2 > g3 > g4 > red child; hiding any one group hides it.
    for hidden in [None, Some(1u32), Some(2), Some(3), Some(4)] {
        let mut layers = vec![
            group(1, None, 255, BlendMode::Normal),
            group(2, Some(1), 255, BlendMode::Normal),
            group(3, Some(2), 255, BlendMode::Normal),
            group(4, Some(3), 255, BlendMode::Normal),
            image(5, Some(4), BlendMode::Normal),
        ];
        if let Some(h) = hidden {
            layers[(h - 1) as usize].visible = false;
        }
        let (sprite, cels) = doc(layers, &[(5, RED)]);
        let want = if hidden.is_some() { [0; 4] } else { RED };
        assert_eq!(compose_visible(&sprite, &cels), want, "hidden {hidden:?}");
    }

    // A hidden child inside a visible group at 128: the sibling still
    // renders at the group's opacity.
    let mut hidden_child = image(2, Some(1), BlendMode::Normal);
    hidden_child.visible = false;
    let layers = vec![
        group(1, None, 128, BlendMode::Normal),
        image(3, Some(1), BlendMode::Normal),
        hidden_child,
    ];
    let (sprite, cels) = doc(layers, &[(2, BLUE), (3, RED)]);
    assert_eq!(compose_visible(&sprite, &cels), [255, 0, 0, 128]);
}

#[test]
fn compose_group_opacity_composites_children_first() {
    // AC-09: red over blue inside a group at 128 over white gives pink —
    // the children merge first, so no blue shows through.
    let layers = vec![
        image(0, None, BlendMode::Normal),
        group(1, None, 128, BlendMode::Normal),
        image(2, Some(1), BlendMode::Normal),
        image(3, Some(1), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(0, WHITE), (2, BLUE), (3, RED)]);
    let out = compose_visible(&sprite, &cels);
    assert_eq!(out[0], 255, "{out:?}");
    assert_eq!(out[1], out[2], "no blue shows through: {out:?}");
    assert!((126..=128).contains(&out[1]), "{out:?}");
    assert_eq!(out[3], 255);
}

#[test]
fn compose_group_blend_mode_applies_to_children_composite() {
    // AC-10: a Multiply group over grey multiplies its Normal magenta child
    // with the grey: 128 · 255 / 255 = 128.
    let layers = vec![
        image(0, None, BlendMode::Normal),
        group(1, None, 255, BlendMode::Multiply),
        image(2, Some(1), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(0, GREY), (2, [255, 0, 255, 255])]);
    assert_eq!(compose_visible(&sprite, &cels), [128, 0, 128, 255]);
}

#[test]
fn compose_isolated_group_child_blends_against_group_content() {
    // AC-11: the Multiply red child of a group at 254 blends against the
    // group's empty content, not the grey below: the result is (nearly)
    // red at alpha 254 over grey, not the multiplied dark red (128, 0, 0).
    let layers = vec![
        image(0, None, BlendMode::Normal),
        group(1, None, 254, BlendMode::Normal),
        image(2, Some(1), BlendMode::Multiply),
    ];
    let (sprite, cels) = doc(layers, &[(0, GREY), (2, RED)]);
    let out = compose_visible(&sprite, &cels);
    assert!(out[0] >= 254 && out[1] <= 1 && out[2] <= 1, "{out:?}");
    assert_eq!(out[3], 255);
}

/// Deterministic xorshift generator — the crate carries no `proptest`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn byte(&mut self) -> u8 {
        (self.next() & 0xff) as u8
    }
}

/// A random 4×3 layer tree (contiguous subtrees, bottom-first) of up to
/// `n` layers with random cels in 2 frames. `group_paint` picks each
/// group's opacity and blend mode, `leaf_blend` each image layer's mode.
fn random_tree(
    rng: &mut Rng,
    n: usize,
    group_paint: &mut dyn FnMut(&mut Rng) -> (u8, BlendMode),
    leaf_blend: &mut dyn FnMut(&mut Rng) -> BlendMode,
) -> (Sprite, CelMap) {
    let (w, h) = (4u32, 3u32);
    let mut layers: Vec<Layer> = Vec::new();
    // Open groups, innermost last.
    let mut open: Vec<u32> = Vec::new();
    for id in 0..n as u32 {
        // Close some open groups before placing the next layer.
        while !open.is_empty() && rng.below(3) == 0 {
            open.pop();
        }
        let parent = open.last().copied();
        if rng.below(3) == 0 {
            let (opacity, blend) = group_paint(rng);
            layers.push(group(id, parent, opacity, blend));
            open.push(id);
        } else {
            let mut l = image(id, parent, leaf_blend(rng));
            l.opacity = [255, 200, rng.byte()][rng.below(3) as usize];
            layers.push(l);
        }
    }
    let mut b = Sprite::builder(w, h)
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100));
    for l in layers.iter().cloned() {
        b = b.add_layer(l);
    }
    let sprite = b.build().expect("random sprite builds");
    let mut cels = CelMap::new();
    for l in layers
        .iter()
        .filter(|l| !matches!(l.kind, pincel_core::LayerKind::Group))
    {
        for f in 0..2u32 {
            if rng.below(4) == 0 {
                continue;
            }
            let cw = 1 + rng.below(u64::from(w)) as u32;
            let ch = 1 + rng.below(u64::from(h)) as u32;
            let mut buf = PixelBuffer::empty(cw, ch, ColorMode::Rgba);
            for px in buf.data.as_chunks_mut::<4>().0 {
                let a = [0, 255, rng.byte()][rng.below(3) as usize];
                *px = [rng.byte(), rng.byte(), rng.byte(), a];
            }
            let mut cel = Cel::image(l.id, FrameIndex::new(f), buf);
            cel.position = (rng.below(4) as i32 - 1, rng.below(3) as i32 - 1);
            cel.opacity = [255, 255, rng.byte()][rng.below(3) as usize];
            cels.insert(cel);
        }
    }
    (sprite, cels)
}

fn compose_frame(sprite: &Sprite, cels: &CelMap, frame: u32) -> Vec<u8> {
    let mut out = Vec::new();
    compose(
        sprite,
        cels,
        &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
        &mut out,
    )
    .expect("compose succeeds");
    out
}

#[test]
fn compose_pass_through_groups_with_normal_children_match_ungrouped_layers() {
    // AC-12: groups at 255 / Normal over Normal descendants render within 1
    // per channel of the same image layers with no groups at all.
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    for case in 0..300 {
        let (sprite, cels) =
            random_tree(&mut rng, 8, &mut |_| (255, BlendMode::Normal), &mut |_| {
                BlendMode::Normal
            });
        let mut flat = sprite.clone();
        flat.layers
            .retain(|l| !matches!(l.kind, pincel_core::LayerKind::Group));
        for l in &mut flat.layers {
            l.parent = None;
        }
        for f in 0..2 {
            let got = compose_frame(&sprite, &cels, f);
            let want = compose_frame(&flat, &cels, f);
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                assert!(
                    g.abs_diff(*w) <= 1,
                    "case {case} frame {f} byte {i}: {g} vs {w}"
                );
            }
        }
    }
}

#[test]
fn compose_nested_groups_apply_each_level() {
    // AC-13: three nested groups at 128 over an opaque child → alpha
    // 255 · ½ · ½ · ½ ≈ 32.
    let layers = vec![
        group(1, None, 128, BlendMode::Normal),
        group(2, Some(1), 128, BlendMode::Normal),
        group(3, Some(2), 128, BlendMode::Normal),
        image(4, Some(3), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(4, RED)]);
    let out = compose_visible(&sprite, &cels);
    assert_eq!(&out[0..3], &[255, 0, 0]);
    assert!((30..=34).contains(&out[3]), "{out:?}");

    // Twelve levels, each isolating, render without error.
    let mut layers: Vec<Layer> = (1..=12)
        .map(|i| group(i, (i > 1).then(|| i - 1), 250, BlendMode::Screen))
        .collect();
    layers.push(image(13, Some(12), BlendMode::Normal));
    let (sprite, cels) = doc(layers, &[(13, RED)]);
    let out = compose_visible(&sprite, &cels);
    assert!(out[3] > 0, "{out:?}");
}

#[test]
fn compose_group_at_zero_opacity_contributes_nothing() {
    // AC-14
    let layers = vec![
        image(0, None, BlendMode::Normal),
        group(1, None, 0, BlendMode::Normal),
        image(2, Some(1), BlendMode::Normal),
        group(3, Some(1), 255, BlendMode::Multiply),
        image(4, Some(3), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(0, GREY), (2, RED), (4, BLUE)]);
    assert_eq!(compose_visible(&sprite, &cels), GREY);
}

#[test]
fn compose_all_layers_filter_folds_hidden_groups() {
    // AC-15: under "all layers" a hidden group at 128 still applies its
    // opacity; under the visible filter it hides its child.
    let mut g = group(1, None, 128, BlendMode::Normal);
    g.visible = false;
    let (sprite, cels) = doc(vec![g, image(2, Some(1), BlendMode::Normal)], &[(2, RED)]);
    assert_eq!(
        compose_with(&sprite, &cels, LayerFilter::All),
        [255, 0, 0, 128]
    );
    assert_eq!(compose_visible(&sprite, &cels), [0; 4]);
}

#[test]
fn compose_only_filter_folds_selected_groups_only() {
    // AC-16: the group applies its opacity only when it is selected.
    let layers = vec![
        group(1, None, 128, BlendMode::Normal),
        image(2, Some(1), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(2, RED)]);
    let both = LayerFilter::Only(vec![LayerId::new(1), LayerId::new(2)]);
    assert_eq!(compose_with(&sprite, &cels, both), [255, 0, 0, 128]);
    let child = LayerFilter::Only(vec![LayerId::new(2)]);
    assert_eq!(compose_with(&sprite, &cels, child), RED);
    let group_only = LayerFilter::Only(vec![LayerId::new(1)]);
    assert_eq!(compose_with(&sprite, &cels, group_only), [0; 4]);
}

/// A random document with isolating groups and linked cels.
fn grouped_linked_doc(rng: &mut Rng) -> (Sprite, CelMap) {
    let (sprite, mut cels) = random_tree(
        rng,
        9,
        &mut |r| {
            let opacity = [255, 128, r.byte()][r.below(3) as usize];
            (
                opacity,
                BlendMode::ALL[r.below(BlendMode::ALL.len() as u64) as usize],
            )
        },
        &mut |r| BlendMode::ALL[r.below(BlendMode::ALL.len() as u64) as usize],
    );
    // Frame 1 links to frame 0 wherever both frames have an image cel.
    let ids: Vec<LayerId> = sprite.layers.iter().map(|l| l.id).collect();
    for id in ids {
        if cels.get(id, FrameIndex::new(0)).is_some()
            && let Some(c) = cels.get(id, FrameIndex::new(1))
            && rng.below(2) == 0
        {
            let mut link = c.clone();
            link.data = CelData::Linked(FrameIndex::new(0));
            cels.insert(link);
        }
    }
    (sprite, cels)
}

#[test]
fn compose_subviewport_and_dirty_hint_match_full_frame_with_groups_and_links() {
    // AC-17: every requested region (viewport ∩ dirty hint, zoomed) equals
    // the matching pixels of one big render around the whole canvas.
    let mut rng = Rng(0x0dd_ba11_cafe_f00d);
    for case in 0..150 {
        let (sprite, cels) = grouped_linked_doc(&mut rng);
        let frame = FrameIndex::new(rng.below(2) as u32);
        let big = Rect::new(-4, -4, sprite.width + 8, sprite.height + 8);
        let onion = (rng.below(2) == 0).then_some(OnionSkin {
            previous_alpha: 180,
            next_alpha: 90,
            frames_back: 1,
            frames_forward: 1,
        });
        let base = ComposeRequest {
            viewport: big,
            onion_skin: onion,
            ..ComposeRequest::full(frame, sprite.width, sprite.height)
        };
        let mut full = Vec::new();
        compose(&sprite, &cels, &base, &mut full).expect("full render");
        for _ in 0..4 {
            let viewport = Rect::new(
                rng.below(5) as i32 - 2,
                rng.below(4) as i32 - 2,
                1 + rng.below(5) as u32,
                1 + rng.below(4) as u32,
            );
            let dirty_hint = (rng.below(2) == 0).then(|| {
                Rect::new(
                    rng.below(4) as i32 - 1,
                    rng.below(3) as i32 - 1,
                    1 + rng.below(3) as u32,
                    1 + rng.below(3) as u32,
                )
            });
            let zoom = 1 + rng.below(3) as u32;
            let req = ComposeRequest {
                viewport,
                dirty_hint,
                zoom,
                ..base.clone()
            };
            let mut out = Vec::new();
            let r = compose(&sprite, &cels, &req, &mut out).expect("region render");
            let rect = r.dirty_rect;
            for y in 0..r.height {
                for x in 0..r.width {
                    let sx = rect.x + (x / zoom) as i32 - big.x;
                    let sy = rect.y + (y / zoom) as i32 - big.y;
                    let fi = (sy as usize * big.width as usize + sx as usize) * 4;
                    let oi = (y as usize * r.width as usize + x as usize) * 4;
                    assert_eq!(
                        &out[oi..oi + 4],
                        &full[fi..fi + 4],
                        "case {case} {req:?} at ({x}, {y})"
                    );
                }
            }
        }
    }
}

#[test]
fn nested_groups_compose_the_same_after_aseprite_round_trip() {
    // AC-18: nested groups with non-default opacity and blend modes (and
    // links) keep every frame's pixels through write → read.
    let mut rng = Rng(0xfeed_beef_0bad_c0de);
    let mut checked = 0;
    for _ in 0..60 {
        let (sprite, cels) = grouped_linked_doc(&mut rng);
        let isolating = sprite.layers.iter().any(|l| {
            matches!(l.kind, pincel_core::LayerKind::Group)
                && (l.opacity != 255 || l.blend_mode != BlendMode::Normal)
        });
        let nested = sprite
            .layers
            .iter()
            .any(|l| matches!(l.kind, pincel_core::LayerKind::Group) && l.parent.is_some());
        let mut bytes = Vec::new();
        write_aseprite(&sprite, &cels, &mut bytes).expect("writer accepts the document");
        let back = read_aseprite(&bytes).expect("reader accepts our output");
        for f in 0..2 {
            assert_eq!(
                compose_frame(&back.sprite, &back.cels, f),
                compose_frame(&sprite, &cels, f),
                "frame {f}"
            );
        }
        if isolating && nested {
            checked += 1;
        }
    }
    assert!(
        checked >= 5,
        "only {checked} documents had nested isolating groups"
    );
}

#[test]
fn compose_inconsistent_hierarchy_returns_without_panicking() {
    // AC-19: a parent cycle between two groups (one holding a child), a
    // layer whose parent names no layer, and a non-contiguous subtree.
    let mut a = group(1, Some(2), 128, BlendMode::Multiply);
    a.parent = Some(LayerId::new(2));
    let b = group(2, Some(1), 128, BlendMode::Normal);
    let in_cycle = image(3, Some(1), BlendMode::Normal);
    let dangling = image(4, Some(99), BlendMode::Normal);
    let (sprite, cels) = doc(vec![a, b, in_cycle, dangling], &[(3, BLUE), (4, RED)]);
    let filters = [
        LayerFilter::Visible,
        LayerFilter::All,
        LayerFilter::Only(vec![LayerId::new(3), LayerId::new(4)]),
        LayerFilter::Only(vec![LayerId::new(1), LayerId::new(3), LayerId::new(4)]),
    ];
    for filter in filters {
        let mut out = Vec::new();
        let req = ComposeRequest {
            include_layers: filter.clone(),
            ..ComposeRequest::full(FrameIndex::new(0), 1, 1)
        };
        let r: Result<_, RenderError> = compose(&sprite, &cels, &req, &mut out);
        assert!(r.is_ok(), "{filter:?}: {r:?}");
        // The dangling layer is on top and renders as a top-level layer.
        assert_eq!(out, RED, "{filter:?}");
    }

    // A subtree split by an unrelated layer: still returns.
    let layers = vec![
        group(1, None, 100, BlendMode::Screen),
        image(2, Some(1), BlendMode::Normal),
        image(3, None, BlendMode::Normal),
        image(4, Some(1), BlendMode::Normal),
    ];
    let (sprite, cels) = doc(layers, &[(2, RED), (3, GREY), (4, BLUE)]);
    let mut out = Vec::new();
    compose(
        &sprite,
        &cels,
        &ComposeRequest::full(FrameIndex::new(0), 1, 1),
        &mut out,
    )
    .expect("non-contiguous subtree composes");
}

#[test]
fn compose_pass_through_group_lets_multiply_child_reach_backdrop() {
    // AC-20: a 255 / Normal group passes its Multiply child through to the
    // grey below: 128 · 255 / 255 = 128, 128 · 0 = 0.
    let layers = vec![
        image(0, None, BlendMode::Normal),
        group(1, None, 255, BlendMode::Normal),
        image(2, Some(1), BlendMode::Multiply),
    ];
    let (sprite, cels) = doc(layers, &[(0, GREY), (2, RED)]);
    assert_eq!(compose_visible(&sprite, &cels), [128, 0, 0, 255]);
}
