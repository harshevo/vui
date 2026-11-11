//! Alerts, badges, chips, a number input, an accordion, skeletons and an empty state.

use super::*;

pub fn view(g: &Gallery) -> Div<Msg> {
    page(
        "Feedback",
        (
            section(
                "Alerts",
                column().gap(12.0).children((
                    alert(AlertKind::Info, "A new version is ready", "Restart to update. Your work is kept."),
                    alert(AlertKind::Success, "Payment received", ""),
                    alert_with(AlertKind::Warning, "Storage almost full", "You have used 92% of your 50 GB.", (button("Manage storage").small(),)),
                    alert(AlertKind::Danger, "Could not sync", "Check your connection and try again."),
                )),
            ),
            section(
                "Badges and chips",
                column().gap(16.0).children((
                    flow().children((badge("Neutral"), badge("Accent").accent(), badge("Paid").success(), badge("Pending").warning(), badge("Failed").danger(), badge("Beta").purple())),
                    flow().gap(8.0).children(g.filters.iter().enumerate().map(|(i, f)| chip(*f, Some(Msg::Unfilter(i))).key(*f)).collect::<Vec<_>>()),
                )),
            ),
            section("Number input", column().width(160.0).child(number_input(g.count, 0.0, 10.0, 1.0, Msg::Count).key("count"))),
            section(
                "Accordion",
                accordion((
                    disclosure("What is vui?", g.section == Some(0), Msg::Section(0), (label("A GPU-drawn UI framework for Rust, built from scratch on wgpu.").color(Tone::Text2),)),
                    disclosure("Does it work without a GPU?", g.section == Some(1), Msg::Section(1), (label("Yes: a CPU renderer draws the same scene.").color(Tone::Text2),)),
                    disclosure("Can I write my own widgets?", g.section == Some(2), Msg::Section(2), (label("Implement one trait.").color(Tone::Text2),)),
                )),
            ),
            section(
                "Loading",
                row().gap(16.0).align_items(Align::Start).children((
                    skeleton(px(40.0), 40.0).rounded(Radius::Pill),
                    column().grow(1.0).gap(12.0).children((skeleton(px(160.0), 14.0), skeleton_text(3))),
                )),
            ),
            section(
                "Empty state",
                card().child(empty_state(icon_set::FOLDER, "No projects yet", "Projects you create or join show up here.", (button("New project").primary().icon(icon_set::PLUS),))),
            ),
        ),
    )
}
