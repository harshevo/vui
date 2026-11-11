//! Tabs, breadcrumbs, a sortable table, a tree and pagination.

use super::*;

/// A file for the table: name, kind, size in KB, edited.
type File = (&'static str, &'static str, u32, &'static str);

const FILES: [File; 5] = [
    ("Roadmap.md", "Markdown", 12, "Today"),
    ("Brand.fig", "Design", 4_820, "Yesterday"),
    ("Q3 report.pdf", "PDF", 1_204, "Mon"),
    ("hero.png", "Image", 2_310, "Oct 28"),
    ("notes.txt", "Text", 3, "Oct 12"),
];

/// The tree: (label, depth, has children, parent).
const TREE: [(&str, usize, bool, Option<usize>); 7] = [
    ("src", 0, true, None),
    ("main.rs", 1, false, Some(0)),
    ("ui", 1, true, Some(0)),
    ("button.rs", 2, false, Some(2)),
    ("table.rs", 2, false, Some(2)),
    ("tests", 0, true, None),
    ("Cargo.toml", 0, false, None),
];

fn sorted(g: &Gallery) -> Vec<(usize, File)> {
    let mut rows: Vec<_> = FILES.iter().copied().enumerate().collect();
    match g.sort_by.0 {
        0 => rows.sort_by_key(|(_, f)| f.0.to_lowercase()),
        1 => rows.sort_by_key(|(_, f)| f.1),
        _ => rows.sort_by_key(|(_, f)| f.2),
    }
    if g.sort_by.1 {
        rows.reverse();
    }
    rows
}

fn shown(g: &Gallery, i: usize) -> bool {
    let mut at = TREE[i].3;
    while let Some(p) = at {
        if !g.unfolded.contains(&p) {
            return false;
        }
        at = TREE[p].3;
    }
    true
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let dir = |c: usize| (g.sort_by.0 == c).then_some(if g.sort_by.1 { SortDir::Descending } else { SortDir::Ascending });
    let table = data_table(
        vec![
            table_column("Name").flex(2.0).sortable(dir(0), Msg::SortBy(0)),
            table_column("Kind").hide_below(Breakpoint::Medium).sortable(dir(1), Msg::SortBy(1)),
            table_column("Size").width(96.0).numeric().sortable(dir(2), Msg::SortBy(2)),
            table_column("Edited").width(96.0).numeric().hide_below(Breakpoint::Medium),
        ],
        sorted(g).into_iter().map(|(i, (name, kind, kb, when))| {
            let size = if kb >= 1000 { format!("{:.1} MB", kb as f32 / 1000.0) } else { format!("{kb} KB") };
            table_row((label(name).weight(500), badge(kind), label(size).color(Tone::Text2), label(when).color(Tone::Text2)))
                .key(i)
                .name(name)
                .selected(g.picked == i)
                .on_click(Msg::Pick(i))
        }),
    );
    let rows: Vec<_> = (0..TREE.len())
        .filter(|&i| shown(g, i))
        .map(|i| {
            let (name, depth, branch, _) = TREE[i];
            let open = branch.then(|| g.unfolded.contains(&i));
            tree_row(name, depth, open, branch.then_some(Msg::Fold(i)))
                .icon(if branch { icon_set::FOLDER } else { icon_set::FILE })
                .selected(i == 4)
                .key(format!("tree-{i}"))
        })
        .collect();
    page(
        "Data",
        (
            section(
                "Tabs",
                column().children((
                    tabs_keyed("tabs", [(0, "Overview".to_string()), (1, "Activity".to_string()), (2, "Settings".to_string())], &g.tab, Msg::Tab),
                    tab_panel((label(["The overview of the project.", "Everything that happened lately.", "How the project is set up."][g.tab]).color(Tone::Text2),)),
                )),
            ),
            section("Breadcrumbs", breadcrumbs([("Home".to_string(), Some(Msg::Tab(0))), ("Projects".to_string(), Some(Msg::Tab(0))), ("vui".to_string(), None)])),
            section("Table", table),
            section("Tree", column().width(280.0).child(tree(rows))),
            section("Pagination", pagination(g.page_no, 12, Msg::PageTo)),
        ),
    )
}
