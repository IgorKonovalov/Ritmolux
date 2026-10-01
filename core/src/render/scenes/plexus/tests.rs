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

/// **A node and the end of an edge at the same point blur by the same amount,
/// because both take it from one function.** Each 3D pipeline's compiled WGSL
/// holds exactly one `coc` definition — the shared camera's — and calls it; a
/// second, local copy would be what let the two drift.
#[test]
fn nodes_and_edges_blur_through_the_one_circle_of_confusion() {
    use crate::render::camera::CAMERA_WGSL;
    use crate::render::scenes::lines::renderer::seg3d_shader_source;
    use crate::render::scenes::marks::quad3d_shader_source;

    let shared = CAMERA_WGSL
        .split("fn coc(")
        .nth(1)
        .expect("the camera defines coc()");
    for (name, source) in [
        ("seg3d", seg3d_shader_source()),
        ("quad3d", quad3d_shader_source()),
    ] {
        assert_eq!(
            source.matches("fn coc(").count(),
            1,
            "{name} must compile exactly one coc(), the camera's"
        );
        assert!(
            source.contains(shared),
            "{name}'s coc() is not the camera's text"
        );
        assert!(source.contains("coc(cam, "), "{name} never calls coc()");
    }
}

fn camera_view() -> crate::render::camera::CameraView {
    crate::render::camera::Camera3d {
        yaw: 0.3,
        pitch: 0.25,
        distance: 3.5,
        fov: 0.8,
        focus: 0.5,
        aperture: 0.0,
    }
    .view(16.0 / 9.0, 1.0, [0.0, 0.0])
}

/// `node_size = 0` issues **no** sprites, rather than sprites of zero size; a
/// positive size issues one per visible point, coloured from its depth and
/// dimmed by its face fade.
#[test]
fn a_zero_node_size_draws_no_nodes() {
    let cloud = sim::Cloud::seeded(5, 50, 50);
    let cam = camera_view();
    let mut out = Vec::with_capacity(50);
    super::node_instances(&mut out, &cloud.pos, &cloud.fade, &cam, 0.05, 0.0, |_| {
        [1.0; 3]
    });
    assert!(out.is_empty());
    super::node_instances(&mut out, &cloud.pos, &cloud.fade, &cam, 0.05, -1.0, |_| {
        [1.0; 3]
    });
    assert!(out.is_empty(), "a negative size is no size");
    super::node_instances(&mut out, &cloud.pos, &cloud.fade, &cam, 0.05, 2.5, |_| {
        [1.0; 3]
    });
    let lit = cloud.fade.iter().filter(|f| **f > 0.0).count();
    assert!(
        !out.is_empty() && out.len() <= lit,
        "{} nodes of {lit} lit points",
        out.len()
    );
    for node in &out {
        assert_eq!(node.radius, 2.5);
        assert!(node.color[0] <= 1.0 && node.color[0] > 0.0);
    }
}

/// **The palette surface behaves on the depth coordinate as on every other
/// scene**: `palette_steps` cuts the near-to-far ramp into that many flat bands,
/// and `palette_mix` crossfades from palette A at 0 to palette B at 1.
#[test]
fn banding_and_the_crossfade_act_on_the_depth_coordinate() {
    use crate::render::palette::{NamedPalette, Palette, PaletteConfig};
    use crate::render::scenes::common::PaletteParams;

    let palette = Palette::bake_pair(
        &PaletteConfig::Named(NamedPalette::Ember),
        &PaletteConfig::Named(NamedPalette::Ice),
    );
    let mut colour = PaletteParams::new(0.0, 1.0);
    let sweep = |colour: &PaletteParams| -> Vec<[f32; 3]> {
        (0..=200)
            .map(|k| super::depth_colour(&palette, colour, 0.5, 0.9, k as f32 / 200.0))
            .collect()
    };

    let continuous = sweep(&colour);
    let mut distinct: Vec<[f32; 3]> = continuous.clone();
    distinct.dedup();
    assert!(distinct.len() > 50, "unbanded, depth is a smooth ramp");

    assert!(colour.set("palette_steps", 4.0));
    let mut banded = sweep(&colour);
    banded.dedup();
    assert!(
        (2..=5).contains(&banded.len()),
        "four steps band the ramp into about four colours, got {}",
        banded.len()
    );

    let mut colour = PaletteParams::new(0.0, 1.0);
    colour.set("palette_mix", 0.0);
    let a = super::depth_colour(&palette, &colour, 0.5, 0.9, 0.3);
    colour.set("palette_mix", 1.0);
    let b = super::depth_colour(&palette, &colour, 0.5, 0.9, 0.3);
    let coord = 0.5 + (0.3 - 0.5) * 0.9;
    let expect = |mix: f32| crate::render::palette::desaturate(palette.sample(coord, mix), 1.0);
    assert_eq!(a, expect(0.0), "mix 0 is palette A");
    assert_eq!(b, expect(1.0), "mix 1 is palette B");
    assert_ne!(a, b, "the two palettes differ at this depth");
}

/// A sheet advanced for a second at `drift` 0.5 with `wave` and `wave_scale`,
/// and its edge count at `link`.
fn rippled(wave: f32, link: f32) -> (sim::Sheet, usize) {
    let mut sheet = sim::Sheet::seeded(9, 400, 400);
    for _ in 0..60 {
        sheet.step(1.0 / 60.0, 0.5, wave, 1.0);
    }
    let mut edges = Vec::with_capacity(20_000);
    let found = sim::link(&sheet.pos, &sheet.fade, link, 20_000, &mut edges);
    (sheet, found)
}

fn rms_height(sheet: &sim::Sheet) -> f32 {
    let sum: f32 = sheet.pos.iter().map(|p| p[1] * p[1]).sum();
    (sum / sheet.pos.len() as f32).sqrt()
}

/// At `wave = 0` every sheet point lies on the plane, and raising `wave`
/// raises the RMS displacement of the point set monotonically.
#[test]
fn the_sheet_lies_flat_at_zero_wave_and_rises_with_it() {
    let (flat, _) = rippled(0.0, 0.3);
    assert!(flat.pos.iter().all(|p| p[1] == 0.0), "wave 0 is the plane");
    let mut prev = 0.0;
    for k in 1..=8 {
        let (sheet, _) = rippled(0.075 * k as f32, 0.3);
        let rms = rms_height(&sheet);
        assert!(
            rms > prev,
            "wave {}: rms {rms} after {prev}",
            0.075 * k as f32
        );
        prev = rms;
    }
}

/// **The ripple changes the mesh gradually**: at a fixed link distance, the
/// edge count at `wave = 0` and at a small wave differ by less than the count
/// at `wave = 0` and at a large one.
#[test]
fn a_small_ripple_moves_the_edge_count_less_than_a_large_one() {
    let link = 0.3;
    let (_, still) = rippled(0.0, link);
    let (_, small) = rippled(0.03, link);
    let (_, large) = rippled(0.6, link);
    println!("edges at wave 0 / 0.03 / 0.6: {still} / {small} / {large}");
    assert!(still > 0, "the flat sheet links");
    assert!(
        still.abs_diff(small) < still.abs_diff(large),
        "{still} / {small} / {large}"
    );
}

/// Only the displacement moves: every point keeps its place in the plane, so
/// neighbours stay neighbours.
#[test]
fn the_sheet_ripples_rather_than_rewires() {
    let mut sheet = sim::Sheet::seeded(4, 100, 100);
    let plane: Vec<[f32; 2]> = sheet.pos.iter().map(|p| [p[0], p[2]]).collect();
    for _ in 0..120 {
        sheet.step(1.0 / 60.0, 1.0, 0.4, 0.7);
    }
    let after: Vec<[f32; 2]> = sheet.pos.iter().map(|p| [p[0], p[2]]).collect();
    assert_eq!(plane, after);
    assert!(sheet.pos.iter().any(|p| p[1] != 0.0), "and it did ripple");
}

/// A `[plexus] layout` the loader does not know is rejected, and the message
/// lists the layouts it does.
#[test]
fn an_unknown_layout_is_rejected_with_the_roster() {
    let err = match Preset::from_toml_str(
        "system = \"plexus\"\nname = \"bad\"\n[plexus]\nlayout = \"torus\"\n",
    ) {
        Ok(_) => panic!("an unknown layout loaded"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("torus"), "{err}");
    assert!(err.contains("cloud, sheet"), "{err}");
    assert!(
        Preset::from_toml_str("system = \"plexus\"\nname = \"ok\"\n[plexus]\nlayout = \"sheet\"\n")
            .is_ok()
    );
}
