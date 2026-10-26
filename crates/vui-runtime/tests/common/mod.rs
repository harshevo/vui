#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use vui_core::{Size, size};
use vui_runtime::testing::TestUi;
use vui_runtime::{Base, Dirty, Env, Widget, builder};

/// The messages the tests' elements send.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Click(&'static str),
    Press(&'static str),
    Double(&'static str),
    Menu(&'static str),
    Hover(&'static str, bool),
    Key(&'static str),
    Dropped(usize),
    Value(i32),
    Text(String),
}

pub type Log = Rc<RefCell<Vec<String>>>;

pub fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

pub fn ui(width: f32, height: f32) -> TestUi<Msg> {
    TestUi::new(width, height)
}

/// Drains the log. Layout may ask the same question twice in a row; that is one entry here.
pub fn take(log: &Log) -> Vec<String> {
    let mut entries = std::mem::take(&mut *log.borrow_mut());
    entries.dedup();
    entries
}

/// A widget that writes down what happens to it, to see what reconciliation does.
pub struct Probe {
    pub log: Log,
    pub tag: &'static str,
    pub dirty: Dirty,
    pub size: Size,
}

pub struct ProbeState {
    log: Log,
    tag: &'static str,
}

impl Drop for ProbeState {
    fn drop(&mut self) {
        self.log.borrow_mut().push(format!("drop {}", self.tag));
    }
}

impl<M: 'static> Widget<M> for Probe {
    type State = ProbeState;

    fn create_state(&self) -> ProbeState {
        self.log.borrow_mut().push(format!("create {}", self.tag));
        ProbeState { log: self.log.clone(), tag: self.tag }
    }

    fn update(&self, _old: &Probe, state: &mut ProbeState) -> Dirty {
        state.log.borrow_mut().push(format!("update {}", self.tag));
        self.dirty
    }

    fn measure(
        &self,
        state: &mut ProbeState,
        _env: &Env,
        _known: vui_runtime::layout::Pair<Option<f32>>,
        _avail: vui_runtime::layout::Pair<vui_runtime::layout::Avail>,
    ) -> Size {
        state.log.borrow_mut().push(format!("measure {}", self.tag));
        self.size
    }
}

pub struct ProbeEl<M: 'static> {
    base: Base<M>,
    widget: Probe,
}

builder!(ProbeEl<M> => Probe);

pub fn probe<M: 'static>(log: &Log, tag: &'static str) -> ProbeEl<M> {
    ProbeEl { base: Base::new(), widget: Probe { log: log.clone(), tag, dirty: Dirty::Layout, size: size(40.0, 20.0) } }
}

impl<M: 'static> ProbeEl<M> {
    pub fn dirty(mut self, dirty: Dirty) -> Self {
        self.widget.dirty = dirty;
        self
    }

    pub fn natural(mut self, w: f32, h: f32) -> Self {
        self.widget.size = size(w, h);
        self
    }
}
