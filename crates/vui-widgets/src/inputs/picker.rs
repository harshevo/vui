//! What a combobox and a command palette share: the state of a list that is filtered by typing and
//! moved through with the keyboard, and the fuzzy matching that filters it.

/// The state of a filtered list: the query typed so far, whether the list shows, and which of the
/// matches is highlighted. Keep one in the app and hand it every [`PickerEvent`]:
///
/// ```
/// use vui_widgets::{Picker, PickerEvent};
///
/// let mut fruit = Picker::new();
/// fruit.apply(PickerEvent::Query("ban".into()));
/// assert!(fruit.open);
/// assert_eq!(fruit.apply(PickerEvent::Pick(1, "Banana".into())), Some(1));
/// assert_eq!((fruit.query.as_str(), fruit.open), ("Banana", false));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picker {
    pub query: String,
    pub open: bool,
    /// The highlighted match, counted among the matches shown.
    pub active: usize,
}

/// What a filtered list asks of its [`Picker`].
#[derive(Clone, Debug, PartialEq)]
pub enum PickerEvent {
    /// The query changed: the list opens and the best match is highlighted.
    Query(String),
    /// Highlight this match.
    Highlight(usize),
    Open(bool),
    /// An option was chosen: its index among all the options, and its label.
    Pick(usize, String),
}

impl Picker {
    pub fn new() -> Picker {
        Picker::default()
    }

    /// Updates the state, and returns the index of the option chosen, if one was.
    pub fn apply(&mut self, event: PickerEvent) -> Option<usize> {
        match event {
            PickerEvent::Query(query) => {
                self.query = query;
                self.open = true;
                self.active = 0;
            }
            PickerEvent::Highlight(i) => {
                self.active = i;
                self.open = true;
            }
            PickerEvent::Open(open) => {
                self.open = open;
                self.active = 0;
            }
            PickerEvent::Pick(i, label) => {
                self.query = label;
                self.open = false;
                self.active = 0;
                return Some(i);
            }
        }
        None
    }

    /// Empties the query and closes the list: after a command palette runs a command.
    pub fn reset(&mut self) {
        *self = Picker::default();
    }
}

/// How well `query` matches `text`, if it does: its characters appear in `text` in order, ignoring
/// case. Matches at the start of words, runs of adjacent characters and a match at the very start
/// score higher, and gaps lower, so "ns" finds "New Session" before "Inspections".
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().collect();
    let (mut qi, mut score, mut last) = (0, 0, None::<usize>);
    for (ti, &c) in t.iter().enumerate() {
        if qi == q.len() {
            break;
        }
        if c.to_lowercase().eq(std::iter::once(q[qi])) {
            let word_start = ti == 0 || !t[ti - 1].is_alphanumeric() || (c.is_uppercase() && t[ti - 1].is_lowercase());
            score += 1;
            if word_start {
                score += 8;
            }
            if ti == 0 {
                score += 10;
            }
            match last {
                Some(l) if l + 1 == ti => score += 5,
                Some(l) => score -= ((ti - l - 1) as i32).min(5),
                None => score -= (ti as i32).min(5),
            }
            last = Some(ti);
            qi += 1;
        }
    }
    (qi == q.len()).then_some(score)
}

/// The indices of the `labels` that match `query`, best first; ties keep their order. An empty query
/// matches everything, in order.
pub fn fuzzy_filter<'a>(query: &str, labels: impl IntoIterator<Item = &'a str>) -> Vec<usize> {
    let mut hits: Vec<(usize, i32)> = labels.into_iter().enumerate().filter_map(|(i, l)| fuzzy_score(query, l).map(|s| (i, s))).collect();
    hits.sort_by_key(|&(i, s)| (-s, i));
    hits.into_iter().map(|(i, _)| i).collect()
}

/// Up and Down move the highlight through `count` matches, wrapping at the ends.
pub(crate) fn step(active: usize, count: usize, down: bool) -> usize {
    if count == 0 {
        return 0;
    }
    let active = active.min(count - 1);
    if down { (active + 1) % count } else { (active + count - 1) % count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_starts_beat_letters_in_the_middle() {
        let labels = ["Inspections", "New Session", "Sessions"];
        assert_eq!(fuzzy_filter("ns", labels), vec![1, 0, 2]);
        assert_eq!(fuzzy_filter("ses", labels)[0], 2, "a prefix wins");
        assert!(fuzzy_score("xyz", "New Session").is_none());
    }

    #[test]
    fn an_empty_query_keeps_everything_in_order() {
        assert_eq!(fuzzy_filter("", ["b", "a", "c"]), vec![0, 1, 2]);
        assert_eq!(fuzzy_filter(" ", ["b", "a"]), vec![0, 1]);
    }

    #[test]
    fn case_is_ignored_and_camel_humps_count_as_words() {
        assert!(fuzzy_score("OPEN", "open file").is_some());
        assert!(fuzzy_score("gs", "gitStatus").unwrap() > fuzzy_score("gs", "glass").unwrap());
    }

    #[test]
    fn the_highlight_wraps() {
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(9, 3, true), 0, "a stale index is brought back in range first");
        assert_eq!(step(0, 0, true), 0);
    }
}
