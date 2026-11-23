//! Arranging content: form fields and settings rows, splitters, docking, disclosures and accordions.

mod accordion;
mod dock;
mod form;
mod splitter;

pub use accordion::{accordion, disclosure};
pub use dock::{
    DockDrag, DockEvent, DockMsg, DockPanel, DockTree, Docking, DragSource, DragSourceWidget, MIN_PANE, Node as DockNode, SplitAxis, Zone, dock,
    drag_source, raised,
};
pub use form::{Control, field, setting};
pub use splitter::{Splitter, SplitterWidget, splitter};
