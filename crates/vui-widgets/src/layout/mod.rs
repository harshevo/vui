//! Arranging content: form fields and settings rows, splitters, disclosures and accordions.

mod accordion;
mod form;
mod splitter;

pub use accordion::{accordion, disclosure};
pub use form::{field, setting};
pub use splitter::{Splitter, SplitterWidget, splitter};
