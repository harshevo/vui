use super::*;

struct S(Style);

impl Styled for S {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.0
    }
}

fn s() -> S {
    S(Style::default())
}

#[test]
fn defaults_are_a_stretching_column_that_shrinks() {
    let d = Style::default();
    assert_eq!(d.direction, Direction::Column);
    assert_eq!(d.align_items, Align::Stretch);
    assert_eq!((d.grow, d.shrink), (0.0, 1.0));
    assert!(d.width.is_auto() && d.min_width.is_auto() && d.basis.is_auto());
}

#[test]
fn lengths_resolve_against_their_basis() {
    assert_eq!(px(12.0).resolve(None), Some(12.0));
    assert_eq!(pct(50.0).resolve(Some(200.0)), Some(100.0));
    assert_eq!(pct(50.0).resolve(None), None, "a percentage of an undecided size is auto");
    assert_eq!(AUTO.resolve(Some(200.0)), None);
    assert_eq!(Len::from(8), px(8.0));
}

#[test]
fn size_setters_write_the_style() {
    let st = s().width(px(120.0)).height(pct(50.0)).min_width(0.0).max_height(300).0;
    assert_eq!(st.width, Len::Px(120.0));
    assert_eq!(st.height, Len::Pct(50.0));
    assert_eq!(st.min_width, Len::Px(0.0));
    assert_eq!(st.max_height, Len::Px(300.0));
    assert_eq!(s().square(24.0).0.height, Len::Px(24.0));
    let f = s().fill().0;
    assert_eq!((f.width, f.height), (Len::Pct(100.0), Len::Pct(100.0)));
}

#[test]
fn padding_shorthands_follow_css_order() {
    let both = s().padding((8.0, 16.0)).0.padding;
    assert_eq!((both.top, both.right, both.bottom, both.left), (8.0, 16.0, 8.0, 16.0));
    let four = s().padding([1.0, 2.0, 3.0, 4.0]).0.padding;
    assert_eq!((four.top, four.right, four.bottom, four.left), (1.0, 2.0, 3.0, 4.0));
    let x = s().padding(4.0).padding_x(10.0).0.padding;
    assert_eq!((x.left, x.right, x.top), (10.0, 10.0, 4.0));
    assert_eq!(Edges::from(Insets::new(1.0, 2.0, 3.0, 4.0)).left, 4.0);
}

#[test]
fn sides_know_their_axes() {
    let e = Edges::from([1.0, 2.0, 3.0, 4.0]);
    assert_eq!((e.start(Axis::X), e.end(Axis::X)), (4.0, 2.0));
    assert_eq!((e.start(Axis::Y), e.end(Axis::Y)), (1.0, 3.0));
    assert_eq!((e.sum(Axis::X), e.sum(Axis::Y)), (6.0, 4.0));
    assert_eq!((e.horizontal(), e.vertical()), (6.0, 4.0));
}

#[test]
fn auto_margins_push_the_element() {
    let m = s().margin(4.0).margin_left_auto().0.margin;
    assert_eq!(m.left, Len::Auto);
    assert_eq!(m.right, Len::Px(4.0));
}

#[test]
fn row_and_column_set_the_direction() {
    assert_eq!(s().column().row().0.direction, Direction::Row);
    assert_eq!(s().row().column().0.direction, Direction::Column);
    assert_eq!(Direction::Row.main_axis(), Axis::X);
    assert_eq!(Axis::X.cross(), Axis::Y);
}

#[test]
fn flex_shares_space_from_a_zero_basis() {
    let st = s().flex(2.0).0;
    assert_eq!((st.grow, st.shrink, st.basis), (2.0, 1.0, Len::Px(0.0)));
}

#[test]
fn inset_pins_every_edge_and_absolute_leaves_the_flow() {
    let st = s().absolute().inset(4.0).0;
    assert_eq!(st.position, Position::Absolute);
    assert_eq!(st.inset.left, Len::Px(4.0));
    assert_eq!(st.inset.bottom, Len::Px(4.0));
}

#[test]
fn the_gap_is_looked_up_by_axis() {
    let st = s().gap(6.0).style(|s| s.gap.y = 10.0).0;
    assert_eq!((st.gap_along(Axis::X), st.gap_along(Axis::Y)), (6.0, 10.0));
}

#[test]
fn centre_sets_both_alignments() {
    let st = s().center().0;
    assert_eq!((st.align_items, st.justify), (Align::Center, Justify::Center));
}
