//! Layer-tree invariant under random structure edits
//! (docs/specs/image-layer-groups.md AC-06).

use pincel_image_core::{
    AddGroup, AddLayer, Command, CommandBus, Document, DuplicateLayer, LayerPlacement, MergeDown,
    MergeGroup, MergeVisible, MoveLayer, MoveLayerTo, RemoveLayer, SetLayerVisible,
};
use proptest::prelude::*;

/// One layer operation; indices are reduced modulo the layer count.
#[derive(Debug, Clone)]
enum Op {
    AddLayer(usize),
    AddGroup,
    Remove(usize),
    Move(usize, usize),
    MoveTo(usize, usize, u8),
    Duplicate(usize),
    MergeDown(usize),
    MergeGroup(usize),
    MergeVisible,
    Hide(usize),
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0usize..16;
    prop_oneof![
        2 => i.clone().prop_map(Op::AddLayer),
        2 => Just(Op::AddGroup),
        1 => i.clone().prop_map(Op::Remove),
        1 => (i.clone(), i.clone()).prop_map(|(a, b)| Op::Move(a, b)),
        4 => (i.clone(), i.clone(), 0u8..3).prop_map(|(a, b, p)| Op::MoveTo(a, b, p)),
        1 => i.clone().prop_map(Op::Duplicate),
        1 => i.clone().prop_map(Op::MergeDown),
        1 => i.clone().prop_map(Op::MergeGroup),
        1 => Just(Op::MergeVisible),
        1 => i.prop_map(Op::Hide),
    ]
}

fn command(op: &Op, n: usize) -> Box<dyn Command> {
    match *op {
        Op::AddLayer(i) => Box::new(AddLayer::above(i % n)),
        Op::AddGroup => Box::new(AddGroup::on_top()),
        Op::Remove(i) => Box::new(RemoveLayer::at(i % n)),
        Op::Move(a, b) => Box::new(MoveLayer::new(a % n, b % n)),
        Op::MoveTo(a, b, p) => {
            let place = [
                LayerPlacement::Above,
                LayerPlacement::Below,
                LayerPlacement::Into,
            ][usize::from(p)];
            Box::new(MoveLayerTo::new(a % n, b % n, place))
        }
        Op::Duplicate(i) => Box::new(DuplicateLayer::at(i % n)),
        Op::MergeDown(i) => Box::new(MergeDown::at(i % n)),
        Op::MergeGroup(i) => Box::new(MergeGroup::at(i % n)),
        Op::MergeVisible => Box::new(MergeVisible::new()),
        Op::Hide(i) => Box::new(SetLayerVisible::new(i % n, false)),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn layer_tree_stays_well_formed_and_undo_restores_it(ops in proptest::collection::vec(op(), 1..40)) {
        let mut bus = CommandBus::new(Document::new(3, 3).expect("doc"));
        let original = bus.document.layers().to_vec();
        for op in &ops {
            let n = bus.document.layer_count();
            let before = bus.document.layers().to_vec();
            let depth = bus.history.undo_depth();
            if bus.apply(command(op, n)).is_err() {
                // A refused operation leaves the tree as it was.
                prop_assert_eq!(bus.document.layers(), before.as_slice(), "{:?}", op);
                prop_assert_eq!(bus.history.undo_depth(), depth);
            }
            prop_assert!(bus.document.is_well_formed(), "{:?} broke the tree", op);
            prop_assert!(bus.document.raster_count() >= 1);
        }
        while bus.undo().expect("undo") {
            prop_assert!(bus.document.is_well_formed());
        }
        prop_assert_eq!(bus.document.layers(), original.as_slice());
    }
}
