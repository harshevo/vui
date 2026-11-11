//! Range sliders, upright sliders, marks and the value bubble.

use vui_core::{NamedKey, Scene, pt};
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Role, Styled, row};
use vui_widgets::{range_slider, slider};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Price(f64, f64),
    Level(f64),
}

struct Shop {
    price: (f64, f64),
    level: f64,
}

impl App for Shop {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Price(lo, hi) => self.price = (lo, hi),
            Msg::Level(v) => self.level = v,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        row().padding(40.0).gap(40.0).children((
            range_slider(self.price.0, self.price.1, 0.0, 100.0).step(10.0).gap(10.0).marks().show_value().format(|v| format!("${v}")).key("price").name("Price").width(200.0).on_change(Msg::Price),
            slider(self.level, 0.0, 10.0).step(1.0).vertical().key("level").name("Level").height(200.0).on_change(Msg::Level),
        ))
    }
}

fn shop() -> TestUi<Msg> {
    TestUi::with_app(400.0, 300.0, Shop { price: (20.0, 80.0), level: 5.0 })
}

fn price(t: &TestUi<Msg>) -> (f64, f64) {
    t.app::<Shop>().price
}

/// The window position `fraction` of the way along a 200-point track, thumb centres 8 in from each end.
fn along(start: f32, fraction: f32) -> f32 {
    start + 8.0 + 184.0 * fraction
}

#[test]
fn a_press_takes_the_nearer_thumb_and_the_thumbs_never_cross() {
    let mut t = shop();
    let r = t.rect("price");
    let y = r.y + r.h / 2.0;
    t.press(pt(along(r.x, 0.3), y)).release(pt(along(r.x, 0.3), y));
    assert_eq!(price(&t), (30.0, 80.0), "nearer the low thumb");
    t.press(pt(along(r.x, 0.6), y)).release(pt(along(r.x, 0.6), y));
    assert_eq!(price(&t), (30.0, 60.0), "nearer the high thumb");
    // Drag the low thumb past the high one: it stops a gap short.
    t.press(pt(along(r.x, 0.3), y)).move_to(pt(along(r.x, 0.95), y)).release(pt(along(r.x, 0.95), y));
    assert_eq!(price(&t), (50.0, 60.0));
}

#[test]
fn tab_moves_between_the_thumbs_before_leaving() {
    let mut t = shop();
    t.tab();
    t.key(NamedKey::ArrowRight);
    assert_eq!(price(&t), (30.0, 80.0), "the low thumb has the keys first");
    t.tab();
    t.key(NamedKey::ArrowRight);
    assert_eq!(price(&t), (30.0, 90.0), "Tab handed them to the high thumb");
    t.key(NamedKey::End);
    assert_eq!(price(&t), (30.0, 100.0));
    t.tab();
    t.key(NamedKey::ArrowUp);
    assert_eq!(price(&t), (30.0, 100.0), "the next Tab left the range slider");
    assert_eq!(t.app::<Shop>().level, 6.0, "for the level slider");
    let node = t.ui.access_tree().find("Price").unwrap().clone();
    assert_eq!(node.role, Role::Slider);
    assert_eq!(node.value.as_deref(), Some("$30 to $100"));
}

#[test]
fn an_upright_slider_runs_bottom_to_top() {
    let mut t = shop();
    let r = t.rect("level");
    assert_eq!((r.w, r.h), (20.0, 200.0));
    let x = r.x + r.w / 2.0;
    let top = pt(x, r.y + 8.0);
    t.press(top).release(top);
    assert_eq!(t.app::<Shop>().level, 10.0, "the top is the maximum");
    let low = pt(x, r.bottom() - 8.0 - 184.0 * 0.2);
    t.press(low).release(low);
    assert_eq!(t.app::<Shop>().level, 2.0);
}

fn dots(scene: &Scene) -> usize {
    quads(scene).iter().filter(|q| (q.rect.w - 4.0).abs() < 0.01 && (q.rect.h - 4.0).abs() < 0.01).count()
}

#[test]
fn marks_sit_at_every_step_and_the_value_shows_while_touched() {
    let mut t = shop();
    assert_eq!(dots(&t.scene()), 11, "0 to 100 in tens");
    assert!(!texts(&t.scene()).iter().any(|s| s.starts_with('$')), "no bubble at rest");
    let r = t.rect("price");
    let at = pt(along(r.x, 0.2), r.y + r.h / 2.0);
    t.press(at).settle();
    assert!(texts(&t.scene()).iter().any(|s| s == "$20"), "{:?}", texts(&t.scene()));
}
