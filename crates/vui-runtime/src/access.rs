//! Accessibility vocabulary: what an element is, and how assistive technology can act on it.
//!
//! These are vui's own types. The tree built from them (`Ui::access_tree`) is plain data; the platform
//! backend turns it into whatever the system's accessibility service wants (AccessKit on macOS, which
//! talks to VoiceOver).

use vui_core::Rect;

/// What an element is, to assistive technology.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// Layout only: assistive technology looks through it.
    None,
    Window,
    Button,
    Link,
    Checkbox,
    Switch,
    RadioButton,
    Slider,
    TextInput,
    SearchInput,
    List,
    ListItem,
    Heading,
    /// Static text.
    Label,
    Image,
    Group,
    Separator,
    ScrollView,
    Tab,
    TabList,
    TabPanel,
    Dialog,
    Toolbar,
    Status,
    Menu,
    MenuItem,
    ProgressBar,
}

/// What the app says about an element. Anything left unset is filled in by the widget.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct A11y {
    pub role: Option<Role>,
    /// The name read out, when the visible text is not enough ("Close", not "x").
    pub label: Option<String>,
    pub description: Option<String>,
    /// Left out of the accessibility tree together with everything inside it.
    pub hidden: bool,
}

/// A numeric range, for sliders and progress bars.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AccessRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
}

/// What a widget reports about itself. The element's own [`A11y`] wins where both say something.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessInfo {
    pub role: Option<Role>,
    pub label: Option<String>,
    pub value: Option<String>,
    pub toggled: Option<bool>,
    /// A checkbox that is neither checked nor unchecked: some of what it stands for is on. `toggled`
    /// says which way the next click goes.
    pub mixed: bool,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub range: Option<AccessRange>,
    /// For text input: where the selection starts and ends, as character offsets into `value`. Both
    /// are the same when there is only a caret.
    pub selection: Option<(usize, usize)>,
    /// Actions the widget answers itself, beyond clicking an element that has a click handler.
    pub actions: Vec<AccessActionKind>,
}

/// Something assistive technology asks an element to do.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AccessAction {
    /// Activate: press the button, flip the switch.
    Click,
    Focus,
    Increment,
    Decrement,
    SetValue(String),
    SetNumeric(f64),
    ScrollIntoView,
}

/// Which kinds of action a node offers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccessActionKind {
    Click,
    Focus,
    Increment,
    Decrement,
    SetValue,
    ScrollIntoView,
}

/// One node of the accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    /// 0 is the window; elements are numbered from 1 and keep their number while they live.
    pub id: u64,
    pub role: Role,
    /// What is read out: the element's name, or its visible text.
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    /// Window coordinates, in logical pixels.
    pub bounds: Rect,
    pub children: Vec<u64>,
    pub toggled: Option<bool>,
    pub mixed: bool,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub range: Option<AccessRange>,
    pub selection: Option<(usize, usize)>,
    pub disabled: bool,
    pub focusable: bool,
    pub actions: Vec<AccessActionKind>,
}

/// Everything assistive technology can see, as of now. Rebuilt on demand.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessTree {
    /// The window first, then every other node.
    pub nodes: Vec<AccessNode>,
    /// The focused node, or the window when nothing is.
    pub focus: u64,
}

impl AccessTree {
    pub fn node(&self, id: u64) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The first node with this name.
    pub fn find(&self, name: &str) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.name.as_deref() == Some(name))
    }

    /// All nodes with this role.
    pub fn with_role(&self, role: Role) -> Vec<&AccessNode> {
        self.nodes.iter().filter(|n| n.role == role).collect()
    }
}
