//! `zenvx-compositor [W H [N]]` — print the zone layout for a screen size.
//! The real Smithay Wayland loop (graphical session) configures surfaces with
//! these rectangles; this lets you verify the geometry headlessly.

use zenvx_compositor::{layout, tile};

fn main() {
    let a: Vec<i32> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let (w, h) = (*a.first().unwrap_or(&1920), *a.get(1).unwrap_or(&1080));
    let n = *a.get(2).unwrap_or(&2) as usize;

    let l = layout(w, h);
    println!("screen {w}x{h}");
    println!("  app  zone: {:?}", l.app_zone);
    println!("  term zone: {:?}", l.term_zone);
    for (i, r) in tile(&l.app_zone, n).iter().enumerate() {
        println!("  app surface {i}: {r:?}");
    }
}
