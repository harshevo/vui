//! The kinetic controls, driven the way a person would: clicking and dragging segments, rolling the
//! slot with the wheel, scrubbing and typing a number, holding to confirm and flicking a radial menu.

use vui_core::{Modifiers, NamedKey, Tone, pt};
use vui_runtime::testing::{TestUi, at, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Styled, column};
use vui_widgets::{ButtonVariant, hold_button, radial_menu, scrub, segments, slot, split_button};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    Fast,
    Balanced,
    Deep,
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Mode(Mode),
    Product(&'static str),
    Qty(f64),
    Clear,
    Kill,
    Radial(&'static str),
    CloseRadial,
    Main,
    More,
}

struct Lab {
    mode: Mode,
    product: &'static str,
    qty: Option<f64>,
    radial: Option<vui_core::Point>,
    log: Vec<Msg>,
}

impl Lab {
    fn new() -> Lab {
        Lab { mode: Mode::Fast, product: "MIS", qty: Some(10.0), radial: None, log: Vec::new() }
    }
}

impl App for Lab {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match &msg {
            Msg::Mode(m) => self.mode = *m,
            Msg::Product(p) => self.product = p,
            Msg::Qty(q) => self.qty = Some(*q),
            Msg::Clear => self.qty = None,
            Msg::Radial(_) | Msg::CloseRadial => self.radial = None,
            _ => {}
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let modes = [(Mode::Fast, "Fast"), (Mode::Balanced, "Balanced"), (Mode::Deep, "Deep")];
        let products = [("MIS", "MIS"), ("CNC", "CNC"), ("NRML", "NRML")];
        let ring = self.radial.map(|p| {
            radial_menu(
                p,
                Some("1,840.50".into()),
                vec![("Buy".into(), Some(Tone::Success), Msg::Radial("buy")), ("Alert".into(), None, Msg::Radial("alert")), ("Sell".into(), Some(Tone::Danger), Msg::Radial("sell")), ("Line".into(), None, Msg::Radial("line"))],
                Msg::CloseRadial,
            )
        });
        column().padding(20.0).gap(12.0).children((
            segments(modes, &self.mode, Msg::Mode).key("modes"),
            slot(products, &self.product, Msg::Product).mono().key("product"),
            scrub(self.qty).label("Qty").step(1.0).range(1.0, 1000.0).on_change(Msg::Qty).on_clear(|| Msg::Clear).width(120.0).key("qty"),
            hold_button("Kill switch", Msg::Kill).key("kill"),
            split_button("Analyse", ButtonVariant::Primary, Msg::Main, Msg::More),
            ring,
        ))
    }
}

fn lab() -> TestUi<Msg> {
    let mut ui = TestUi::with_app(480.0, 520.0, Lab::new());
    ui.settle();
    ui
}

fn log(ui: &mut TestUi<Msg>) -> Vec<Msg> {
    ui.update_app(|a: &mut Lab| std::mem::take(&mut a.log))
}

#[test]
fn segments_pick_by_click_and_arrow_and_the_pill_glides() {
    let mut ui = lab();
    let r = ui.rect("modes");
    ui.click(at(r, 0.9, 0.5));
    assert_eq!(log(&mut ui), vec![Msg::Mode(Mode::Deep)]);
    // Clicking the chosen one again sends nothing.
    ui.settle().click(at(r, 0.9, 0.5));
    assert!(log(&mut ui).is_empty());
    // The pill is somewhere between the two while it moves, and settles.
    ui.click(at(r, 0.05, 0.5)).advance(0.02);
    ui.settle();
    assert_eq!(log(&mut ui), vec![Msg::Mode(Mode::Fast)]);
    // The arrow keys move along once it has focus.
    ui.key(NamedKey::ArrowRight);
    assert_eq!(log(&mut ui), vec![Msg::Mode(Mode::Balanced)]);
    for label in ["Fast", "Balanced", "Deep"] {
        assert!(texts(&ui.scene()).iter().any(|t| t == label), "{label} is drawn");
    }
}

#[test]
fn the_slot_rolls_with_the_wheel_and_a_click_wraps() {
    let mut ui = lab();
    let c = ui.center("product");
    ui.wheel(c, 0.0, -24.0);
    assert_eq!(log(&mut ui), vec![Msg::Product("CNC")]);
    ui.settle().wheel(c, 0.0, -24.0);
    assert_eq!(log(&mut ui), vec![Msg::Product("NRML")]);
    // Past the end it bounces and sends nothing.
    ui.settle().wheel(c, 0.0, -24.0);
    assert!(log(&mut ui).is_empty());
    // A click goes round to the first; Shift-click would go back.
    ui.settle().click(c);
    assert_eq!(log(&mut ui), vec![Msg::Product("MIS")]);
    // Only the chosen word shows at rest.
    ui.leave().settle();
    let shown = texts(&ui.scene());
    assert!(shown.iter().any(|t| t == "MIS"));
    assert!(!shown.iter().any(|t| t == "CNC"), "the neighbours hide when the pointer is away: {shown:?}");
}

#[test]
fn scrub_drags_types_and_clears() {
    let mut ui = lab();
    let r = ui.rect("qty");
    let start = at(r, 0.5, 0.5);
    // 20 px right is five steps.
    ui.press(start).move_to(pt(start.x + 20.0, start.y)).release(pt(start.x + 20.0, start.y));
    assert_eq!(log(&mut ui).last(), Some(&Msg::Qty(15.0)));
    // A click starts typing; Return keeps what was typed.
    ui.settle().click(start);
    for _ in 0..4 {
        ui.key(NamedKey::Backspace);
    }
    ui.type_text("42").key(NamedKey::Enter);
    assert_eq!(log(&mut ui), vec![Msg::Qty(42.0)]);
    // What isn't a number stays to be fixed, and sends nothing.
    ui.settle().click(start).type_text("1.2.3").key(NamedKey::Enter);
    assert!(log(&mut ui).is_empty());
    assert!(texts(&ui.scene()).iter().any(|t| t.ends_with("1.2.3")), "kept for fixing");
    ui.key(NamedKey::Escape);
    // Escape throws typing away.
    ui.settle().click(start).type_text("9").key(NamedKey::Escape);
    assert!(log(&mut ui).is_empty());
    // Arrow keys nudge it, Shift by ten.
    ui.key(NamedKey::ArrowUp);
    ui.key_with(NamedKey::ArrowUp, Modifiers::SHIFT);
    assert_eq!(log(&mut ui), vec![Msg::Qty(43.0), Msg::Qty(53.0)]);
    // It stays inside its range.
    ui.settle().press(start).move_to(pt(start.x - 400.0, start.y)).release(pt(start.x - 400.0, start.y));
    assert_eq!(log(&mut ui).last(), Some(&Msg::Qty(1.0)));
    // Backspace on a focused value clears it.
    ui.settle().key(NamedKey::Backspace);
    assert_eq!(log(&mut ui), vec![Msg::Clear]);
}

#[test]
fn hold_fires_only_after_the_hold_and_not_on_an_early_release() {
    let mut ui = lab();
    let c = ui.center("kill");
    // A plain click shakes and says "Hold" for a moment, and sends nothing.
    ui.click(c).advance(0.1);
    assert!(texts(&ui.scene()).iter().any(|t| t == "Hold"), "a click is told to hold");
    ui.advance(1.0);
    assert!(texts(&ui.scene()).iter().any(|t| t == "Kill switch"));
    ui.press(c).advance(0.3).release(c);
    ui.advance(1.0);
    assert!(log(&mut ui).is_empty(), "let go early: nothing happens");
    ui.press(c);
    for _ in 0..45 {
        ui.advance(1.0 / 60.0);
    }
    assert_eq!(log(&mut ui), vec![Msg::Kill], "fires while still held, once");
    ui.advance(0.5).release(c).settle();
    assert!(log(&mut ui).is_empty(), "letting go after it fired sends nothing more");
}

#[test]
fn the_radial_menu_picks_by_direction_and_closes_from_the_middle() {
    let mut ui = lab();
    let p = pt(240.0, 300.0);
    ui.update_app(|a: &mut Lab| a.radial = Some(p));
    ui.settle();
    // Straight up is the first choice; right is the second.
    ui.move_to(pt(p.x, p.y - 60.0)).click(pt(p.x, p.y - 60.0));
    assert_eq!(log(&mut ui), vec![Msg::Radial("buy")]);
    ui.update_app(|a: &mut Lab| a.radial = Some(p));
    ui.settle().click(pt(p.x + 60.0, p.y + 4.0));
    assert_eq!(log(&mut ui), vec![Msg::Radial("alert")]);
    ui.update_app(|a: &mut Lab| a.radial = Some(p));
    ui.settle().click(p);
    assert_eq!(log(&mut ui), vec![Msg::CloseRadial]);
    // Number keys pick clockwise from the top.
    ui.update_app(|a: &mut Lab| a.radial = Some(p));
    ui.settle().type_text("3");
    assert_eq!(log(&mut ui), vec![Msg::Radial("sell")]);
}

#[test]
fn the_split_button_has_two_halves() {
    let mut ui = lab();
    ui.click_key("split-main");
    ui.click_key("split-more");
    assert_eq!(log(&mut ui), vec![Msg::Main, Msg::More]);
}
