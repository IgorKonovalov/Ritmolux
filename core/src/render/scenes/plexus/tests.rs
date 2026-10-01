// Tests index slices and panic on failure; allowed over the file's hot-path
// pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::sim::{self, CLOUD_HALF_EXTENT, Edge, face_fade, link_presence};
use super::{PlexusConfig, PlexusLayout};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;
use crate::render::metrics::{coverage, frame_diff};
use crate::render::{HeadlessOptions, RenderError, Renderer};

/// **An edge's presence is continuous in its pair's distance** — sweeping a
/// pair across `link_distance` gives no step.
///
/// Two points in the interior, where the face fade is exactly 1, are moved
/// apart in steps of `1/2000` of the link distance from half of it to half
/// again past it. `smoothstep(1 - d/L)` has a slope of at most `1.5 / L`, so
/// no two neighbouring samples may differ by more than that slope times the
/// step, with a little float slack; a pop would be a jump of the whole edge.
#[test]
fn an_edge_fades_rather_than_pops_across_the_link_distance() {
    let link = 0.35;
    let steps = 2000;
    let dd = link / steps as f32;
    let max_jump = 1.5 * dd / link * 1.01 + 1e-6;
    let mut edges: Vec<Edge> = Vec::new();
    let mut prev: Option<f32> = None;
    let mut saw_linked = false;
    let mut saw_unlinked = false;
    for k in 0..=(2 * steps) {
        let d = 0.5 * link + k as f32 * dd * 0.5;
        let pos = [[0.0, 0.0, 0.0], [d, 0.0, 0.0]];
        let fade = [face_fade(pos[0]), face_fade(pos[1])];
        assert_eq!(fade, [1.0, 1.0], "the pair has to sit in the interior");
        sim::link(&pos, &fade, link, 8, &mut edges);
        let presence = edges.first().map_or(0.0, |e| e.presence);
        if presence > 0.0 {
            saw_linked = true;
        } else {
            saw_unlinked = true;
        }
        if let Some(prev) = prev {
            assert!(
                (presence - prev).abs() <= max_jump,
                "a step of {} at d = {d}: {prev} -> {presence}",
                (presence - prev).abs()
            );
        }
        prev = Some(presence);
    }
    assert!(
        saw_linked && saw_unlinked,
        "the sweep has to cross the link distance"
    );
    assert_eq!(
        link_presence(link, link),
        0.0,
        "exactly nothing at the link distance"
    );
    assert_eq!(
        link_presence(0.0, link),
        1.0,
        "exactly whole at no distance"
    );
}

/// A point fades to **exactly zero** at a face of the cube, so the jump it
/// takes when it wraps through that face is invisible, and every edge attached
/// to it has already faded with it.
#[test]
fn a_point_fades_out_before_it_wraps() {
    let h = CLOUD_HALF_EXTENT;
    assert_eq!(face_fade([h, 0.0, 0.0]), 0.0);
    assert_eq!(face_fade([0.0, -h, 0.0]), 0.0);
    assert_eq!(face_fade([0.0, 0.0, 0.0]), 1.0);
    // And continuously: approaching the face, the fade falls monotonically.
    let mut prev = 1.0;
    for k in 0..=100 {
        let x = h * k as f32 / 100.0;
        let f = face_fade([x, 0.0, 0.0]);
        assert!(f <= prev + 1e-7, "rose at x = {x}: {prev} -> {f}");
        prev = f;
    }
    // A faded point links to nothing.
    let pos = [[h, 0.0, 0.0], [h - 0.01, 0.0, 0.0]];
    let fade = [face_fade(pos[0]), face_fade(pos[1])];
    let mut edges = Vec::new();
    sim::link(&pos, &fade, 0.3, 8, &mut edges);
    assert!(edges.is_empty(), "{edges:?}");
}

/// The graph is held to the edge cap, and the count it returns says how many
/// pairs linked, so a truncation is visible to the caller.
#[test]
fn the_graph_is_held_to_its_cap_and_says_so() {
    let pos: Vec<[f32; 3]> = (0..10).map(|i| [i as f32 * 0.01, 0.0, 0.0]).collect();
    let fade = vec![1.0; pos.len()];
    let mut edges = Vec::with_capacity(5);
    let found = sim::link(&pos, &fade, 0.5, 5, &mut edges);
    assert_eq!(found, 45, "every pair of ten links");
    assert_eq!(edges.len(), 5);
    assert!(edges.capacity() >= 5);
}

/// Drift moves the points, and `drift = 0` holds them exactly still.
#[test]
fn drift_moves_the_cloud_and_zero_holds_it() {
    let mut cloud = sim::Cloud::seeded(3, 64, 64);
    let start = cloud.pos.clone();
    for _ in 0..60 {
        cloud.step(1.0 / 60.0, 0.0);
    }
    assert_eq!(cloud.pos, start, "drift 0 is still");
    for _ in 0..60 {
        cloud.step(1.0 / 60.0, 0.5);
    }
    let moved = cloud.pos.iter().zip(&start).filter(|(a, b)| a != b).count();
    assert_eq!(moved, start.len(), "every point moved");
    for p in &cloud.pos {
        for x in p {
            assert!(x.abs() <= CLOUD_HALF_EXTENT, "{p:?} left the cube");
        }
    }
}

/// The layout roster parses what it prints, and nothing else.
#[test]
fn the_layout_roster_round_trips() {
    for layout in PlexusLayout::ALL {
        assert_eq!(PlexusLayout::from_name(layout.as_str()), Some(layout));
    }
    assert_eq!(PlexusLayout::from_name("Cloud"), None);
    assert_eq!(PlexusConfig::default().layout, PlexusLayout::Cloud);
}

/// **A plexus preset renders a visible linked network, and it moves**: two
/// captures a second apart differ.
///
/// Rendered through the whole engine at the golden suite's size, from a
/// preset rather than a hand-built scene, so the loader, the configure hook,
/// the camera and the `seg3d` pipeline are all on the path.
#[test]
fn a_plexus_preset_renders_a_moving_network() {
    let preset = Preset::from_toml_str(
        r#"
system = "plexus"
name = "plexus_probe"

[plexus]
points = 200
seed = 7

[params]
link_distance = "0.45"
line_width = "2.0"
drift = "0.3"
brightness = "1.2"
"#,
    )
    .expect("the probe preset loads");
    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 160,
        height: 100,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    renderer.set_presets(vec![preset]);
    let frame = AnalysisFrame::default();
    let one = renderer
        .capture_preset("plexus_probe", &frame, 60)
        .expect("capture at one second");
    let two = renderer
        .capture_preset("plexus_probe", &frame, 120)
        .expect("capture at two seconds");
    let lit = coverage(&one, [0, 0, 0, 255], 8);
    println!(
        "plexus probe: lit fraction {lit:.4}, frame diff {:.4}",
        frame_diff(&one, &two)
    );
    assert!(lit > 0.02, "the network lights {lit} of the frame");
    assert!(lit < 0.9, "the network is lines, not a wash: {lit}");
    assert!(
        frame_diff(&one, &two) > 0.0005,
        "two frames a second apart must differ"
    );
}
