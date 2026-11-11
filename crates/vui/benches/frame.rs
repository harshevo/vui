//! How long a frame takes: building a view, laying it out, painting it into a scene and drawing the
//! scene on the CPU. No harness crate: each case runs a fixed number of times and prints the median.
//!
//! ```sh
//! cargo bench -p vui --bench frame
//! ```

#![allow(clippy::print_stdout, reason = "a benchmark reports on stdout")]

use std::hint::black_box;
use std::time::{Duration, Instant};

use vui::core::{Resources, size};
use vui::prelude::*;
use vui::render::CpuRenderer;
use vui::runtime::Ui;

fn rows(n: usize) -> impl IntoElement<()> {
    column().padding(16.0).gap(4.0).children(
        (0..n)
            .map(|i| {
                row().gap(8.0).padding(6.0).children((
                    label(format!("Row {i}")).grow(1.0),
                    badge(format!("{}", i * 7 % 100)),
                    button("Open"),
                ))
            })
            .collect::<Vec<_>>(),
    )
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

fn bench(name: &str, runs: usize, mut f: impl FnMut()) {
    f(); // warm caches: fonts, glyphs, layout
    let times = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    println!("{name:<36} {:>10.3} ms", median(times).as_secs_f64() * 1e3);
}

fn main() {
    let res = Resources::bundled_only();
    for n in [10, 100, 1000] {
        let mut ui: Ui<()> = Ui::new(res.clone(), Theme::light(), size(800.0, 600.0), 2.0);
        bench(&format!("view + layout + paint, {n} rows"), 20, || {
            ui.set_view(rows(n));
            black_box(ui.paint());
        });
        bench(&format!("repaint unchanged, {n} rows"), 20, || {
            black_box(ui.paint());
        });
    }

    let mut ui: Ui<()> = Ui::new(res.clone(), Theme::dark(), size(800.0, 600.0), 2.0);
    ui.set_view(rows(40));
    let scene = ui.paint();
    let mut cpu = CpuRenderer::new(res);
    bench("cpu render 800x600 @2x", 10, || {
        black_box(cpu.render_to_image(&scene));
    });
}
