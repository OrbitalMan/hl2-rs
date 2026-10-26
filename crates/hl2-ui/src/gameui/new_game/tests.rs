use super::*;
use source_ui::input::{Input, Key};
use std::collections::BTreeMap;

fn fixture(count: usize) -> Dialog {
    let mut loc = Localization::default();
    loc.insert("GameUI_Chapter", "KAPITEL");
    let files: BTreeMap<_, _> = (1..=count)
        .map(|i| {
            loc.insert(&format!("test_Chapter{i}_Title"), &format!("Name {i}"));
            (format!("cfg/chapter{i}.cfg"), format!("map level{i}"))
        })
        .collect();
    let entries = catalog(
        files.keys().map(String::as_str),
        |file| files.get(file).cloned(),
        "test",
        &loc,
    );
    Dialog::new(
        "test",
        Catalog {
            entries,
            ..Default::default()
        },
        &loc,
    )
}

#[test]
fn discovery_uses_config_keys_not_map_tables_and_sorts_fractional_chapters() {
    let mut loc = Localization::default();
    loc.insert("GameUI_Chapter", "KAPITEL");
    loc.insert("mod_Chapter9a_Title", "Verflechtung");
    let files = [
        "cfg/chapter10.cfg",
        "cfg/chapter9a.cfg",
        "cfg/chapter9.cfg",
        "cfg/chapter1.cfg",
        "cfg/chapter2.cfg",
        "cfg/chapter.cfg",
        "cfg/chapter../3.cfg",
    ];
    let entries = catalog(
        files,
        |f| match f {
            "cfg/chapter1.cfg" => Some("playvideo_exitcommand recap map first".into()),
            "cfg/chapter2.cfg" => Some("// map absent".into()),
            _ => Some("map arbitrary".into()),
        },
        "mod",
        &loc,
    );
    assert_eq!(
        entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
        ["1", "9", "9a", "10"]
    );
    assert_eq!(entries[0].map, "first");
    assert_eq!(entries[2].title, "Verflechtung");
    assert_eq!(entries[2].heading, "KAPITEL 9a");
}

#[test]
fn chapter_pointer_capture_selects_on_release_and_start_keeps_game_identity() {
    let mut dialog = fixture(7);
    let canvas = vec2(960., 720.);
    let frame = dialog.frame(canvas);
    let pointer = |rect: Rect| Some(frame.origin + vec2(rect.x + 2., rect.y + 2.) * frame.scale);
    let card = dialog.panel_rect(1);
    assert_eq!(
        dialog.input(
            &Input {
                pointer: pointer(card),
                click: true,
                held: true,
                ..Default::default()
            },
            canvas
        ),
        Outcome::None
    );
    assert_eq!(dialog.selected, 0);
    dialog.input(
        &Input {
            pointer: None,
            held: true,
            ..Default::default()
        },
        canvas,
    );
    dialog.input(
        &Input {
            pointer: pointer(card),
            released: true,
            ..Default::default()
        },
        canvas,
    );
    assert_eq!(dialog.selected, 1);
    assert_eq!(dialog.take_sounds(), ["ui/buttonclickrelease.wav"]);
    let play = dialog
        .targets()
        .find(|(t, _)| *t == Target::Play)
        .unwrap()
        .1;
    dialog.input(
        &Input {
            pointer: pointer(play),
            click: true,
            held: true,
            ..Default::default()
        },
        canvas,
    );
    assert_eq!(
        dialog.input(
            &Input {
                pointer: pointer(play),
                released: true,
                ..Default::default()
            },
            canvas
        ),
        Outcome::Start(crate::console::GameMap::new("test", "level2"))
    );
}

#[test]
fn keyboard_pages_clamps_and_activates_only_on_release() {
    let mut dialog = fixture(7);
    let canvas = vec2(1280., 720.);
    for _ in 0..10 {
        dialog.input(
            &Input {
                keys: [Key::Right].into(),
                ..Default::default()
            },
            canvas,
        );
    }
    assert_eq!((dialog.selected, dialog.first), (6, 6));
    assert!(!dialog.targets().any(|(t, _)| t == Target::Next));
    assert_eq!(
        dialog.input(
            &Input {
                keys: [Key::Enter].into(),
                held_keys: [Key::Enter].into(),
                ..Default::default()
            },
            canvas
        ),
        Outcome::None
    );
    assert_eq!(
        dialog.input(&Input::default(), canvas),
        Outcome::Start(crate::console::GameMap::new("test", "level7"))
    );
    dialog.open();
    assert_eq!((dialog.selected, dialog.first), (0, 0));
}

#[test]
fn cancel_tab_focus_and_scaled_frames_use_shared_controls() {
    let mut dialog = fixture(1);
    let canvas = vec2(1080., 1920.);
    let b = dialog.bounds(canvas);
    assert!(b.x >= 0. && b.y >= 0. && b.x + b.w <= canvas.x && b.y + b.h <= canvas.y);
    // One card, Play, Cancel, Close; Tab follows reading order without a default button.
    for _ in 0..2 {
        dialog.input(
            &Input {
                keys: [Key::Tab].into(),
                ..Default::default()
            },
            canvas,
        );
    }
    assert_eq!(dialog.focus, Some(Target::Cancel));
    assert_eq!(
        dialog.input(
            &Input {
                keys: [Key::Space].into(),
                ..Default::default()
            },
            canvas
        ),
        Outcome::Close
    );
}

#[test]
#[ignore = "requires an owned installation"]
fn owned_chapter_catalogs_use_each_games_configs_localization_and_thumbnails() {
    let root = source_assets::install::discover().expect("owned installation");
    let vfs = source_assets::vpk::Vfs::mount(&root).unwrap();
    let resources = super::super::resources::GameUiResources::load(&vfs);
    for (game, count, first, last) in [
        ("hl2", 15, "d1_trainstation_01", "credits"),
        ("episodic", 5, "ep1_citadel_00", "ep1_c17_05"),
        ("ep2", 7, "ep2_outland_01", "ep2_outland_12a"),
        ("lostcoast", 1, "d2_lostcoast", "d2_lostcoast"),
    ] {
        let c = &resources.new_games[game];
        assert_eq!(c.entries.len(), count, "{game}");
        assert_eq!(c.entries.first().unwrap().map, first, "{game}");
        assert_eq!(c.entries.last().unwrap().map, last, "{game}");
        assert_eq!(
            c.thumbnails.iter().filter(|t| t.is_some()).count(),
            count,
            "{game}"
        );
        assert!(
            c.entries
                .iter()
                .all(|e| !e.title.to_ascii_lowercase().contains("_chapter")),
            "{game}"
        );
        assert!(c.dialog.get("Play").is_some(), "{game}");
    }
}

#[test]
fn padded_chapter_texture_is_clipped_at_native_size_before_ui_scaling() {
    use source_ui::{
        canvas::Canvas,
        widgets::{Fonts, Painter, Theme},
    };
    let mut dialog = fixture(1);
    dialog.textures = vec![Some(Texture2D::from_rgba8(
        256,
        128,
        &vec![255; 256 * 128 * 4],
    ))];
    let canvas = Canvas::default();
    let theme = Theme::default();
    let fonts = Fonts::default();
    let p = Painter {
        canvas: &canvas,
        fonts: &fonts,
        theme: &theme,
        origin: Vec2::ZERO,
        scale: 1.,
    };
    dialog.draw(&p, vec2(1024., 768.));
    let commands = canvas.drain();
    let quad = commands
        .iter()
        .find(|q| q.texture.is_some())
        .expect("chapter thumbnail");
    assert_eq!(quad.source, Rect::new(0., 0., 152., 86.));
    let scale = crate::proportional::dialog_scale(vec2(1024., 768.));
    assert_eq!(quad.destination.w, (152. * scale).round());
    assert_eq!(quad.destination.h, (86. * scale).round());
}

#[test]
fn single_chapter_layout_centers_the_card_and_sizes_decorative_rules_to_the_frame() {
    use source_ui::{
        canvas::Canvas,
        widgets::{Fonts, Painter, Theme},
    };
    let mut dialog = fixture(1);
    dialog.catalog.dialog = Layout::parse(
        br#""dialog" {
        "NewGameDialog" { "ControlName" "CNewGameDialog" "wide" "300" "tall" "296" }
        "divider" { "ControlName" "Divider" "xpos" "24" "ypos" "34" "wide" "548" "tall" "2" }
        "Play" { "ControlName" "Button" "xpos" "63" "ypos" "252" "wide" "124" "tall" "24" }
        "Cancel" { "ControlName" "Button" "xpos" "200" "ypos" "252" "wide" "72" "tall" "24" }
    }"#,
    )
    .unwrap();
    assert_eq!(dialog.panel_rect(0).x, 64.);
    let canvas = Canvas::default();
    let theme = Theme::default();
    let fonts = Fonts::default();
    let size = vec2(1024., 768.);
    dialog.draw(
        &Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        },
        size,
    );
    let bounds = dialog.bounds(size);
    for q in canvas.drain() {
        assert!(q.destination.x >= bounds.x && q.destination.y >= bounds.y);
        assert!(q.destination.x + q.destination.w <= bounds.x + bounds.w + 1.);
        assert!(q.destination.y + q.destination.h <= bounds.y + bounds.h + 1.);
    }
}
