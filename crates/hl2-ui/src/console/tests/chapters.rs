use super::*;

fn open_and_start(console: &mut Console) -> Vec<Effect> {
    let effects = click_row(console, 0);
    assert_eq!(
        effects,
        vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
    );
    assert_eq!(console.mode(), Mode::NewGame);
    assert!(
        !console.is_loading(),
        "opening chapter selection must not load gameplay"
    );
    console.input(&Input {
        keys: [Key::Enter].into(),
        ..Default::default()
    })
}

#[test]
fn background_level_main_menu_navigation_new_game_and_quit_box() {
    let mut console = installed_console();
    console.level_loaded(true);
    assert_eq!(console.mode(), Mode::MainMenu);
    console.escape();
    assert_eq!(console.mode(), Mode::MainMenu, "nothing to resume");
    click_row(&mut console, 0);
    assert_eq!(console.main_menu.category(), "hl2");
    console.escape();
    assert_eq!(console.main_menu.category(), "main");
    assert_eq!(console.mode(), Mode::MainMenu);
    click_row(&mut console, 0);
    assert_eq!(
        open_and_start(&mut console),
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::ChapterMap(GameMap::new("hl2", "d1_trainstation_01")),
        ]
    );
    console.level_loaded(false);
    assert_eq!(console.mode(), Mode::Gameplay);
    assert_eq!(console.main_menu.category(), "main");
    console.escape();
    assert_eq!(console.mode(), Mode::Pause);

    console.level_loaded(true);
    click_row(&mut console, 6);
    let (quit, kind) = console.confirm_box.clone().expect("quit confirmation");
    assert_eq!(kind, ConfirmKind::Quit);
    let (origin, scale, layout) = console.quit_geometry(&quit);
    let click_btn = |console: &mut Console, button: Rect| {
        let pointer = Some(origin + vec2(button.x + 3., button.y + 3.) * scale);
        let _ = console.input(&Input {
            pointer,
            click: true,
            held: true,
            ..Default::default()
        });
        console.input(&Input {
            pointer,
            released: true,
            ..Default::default()
        })
    };
    assert_eq!(
        click_btn(&mut console, layout.buttons[1]),
        vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
    );
    assert!(console.confirm_box.is_none(), "Cancel closes the box");
    assert_eq!(console.mode(), Mode::MainMenu);
    click_row(&mut console, 6);
    assert_eq!(
        click_btn(&mut console, layout.buttons[0]),
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::Quit,
        ]
    );
}

#[test]
fn episode_one_and_two_new_game_navigation_and_missing_content_error_recovery() {
    let mut console = installed_console();
    console.level_loaded(true);
    assert_eq!(console.mode(), Mode::MainMenu);

    // Row 1 on main menu: HL2: EPISODE ONE
    assert_eq!(
        click_row(&mut console, 1),
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::MainMenuMap(GameMap::new("episodic", "ep1_background01")),
        ]
    );
    assert_eq!(console.main_menu.category(), "episodic");
    console.level_loaded(true);

    // Row 0 in Episode One submenu: NEW GAME
    let effects = open_and_start(&mut console);
    assert_eq!(
        effects,
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::ChapterMap(GameMap::new("episodic", "ep1_citadel_00")),
        ]
    );
    assert!(console.is_loading());
    assert_eq!(
        console.loading_dialog.as_ref().unwrap().map,
        "ep1_citadel_00"
    );

    // Graceful error handling for missing episode content:
    // Loader reports failure, error is latched onto loading dialog.
    console.loading_failed("owned map ep1_citadel_00.bsp not found");
    assert!(!console.is_loading());
    assert!(console.loading_dialog.as_ref().unwrap().is_failed());
    assert_eq!(
        console.loading_dialog.as_ref().unwrap().error.as_deref(),
        Some("owned map ep1_citadel_00.bsp not found")
    );

    // Player dismisses failed dialog via Escape and returns to main menu
    console.escape();
    assert!(console.loading_dialog.is_none());
    assert_eq!(console.mode(), Mode::MainMenu);

    // Row 2 on main menu: HL2: EPISODE TWO
    assert_eq!(
        click_row(&mut console, 2),
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::MainMenuMap(GameMap::new("ep2", "ep2_background01")),
        ]
    );
    assert_eq!(console.main_menu.category(), "ep2");
    console.level_loaded(true);

    // Row 0 in Episode Two submenu: NEW GAME
    let effects = open_and_start(&mut console);
    assert_eq!(
        effects,
        vec![
            Effect::PlaySound("ui/buttonclickrelease.wav".into()),
            Effect::ChapterMap(GameMap::new("ep2", "ep2_outland_01")),
        ]
    );
    assert!(console.is_loading());
    assert_eq!(
        console.loading_dialog.as_ref().unwrap().map,
        "ep2_outland_01"
    );

    // Missing episode content error is dismissed via Enter input
    console.loading_failed("owned map ep2_outland_01.bsp not found");
    let enter = Input {
        keys: [Key::Enter].into_iter().collect(),
        ..Default::default()
    };
    console.input(&enter);
    assert!(console.loading_dialog.is_none());
    assert_eq!(console.mode(), Mode::MainMenu);
}

#[test]
fn lostcoast_navigation_and_missing_content_error_recovery() {
    let mut console = installed_console();
    console.set_mode(Mode::MainMenu);
    console.background_level = true;
    console.loaded_map = Some(GameMap::new("hl2", "background01"));

    // In main menu: row 4 is EXTRAS
    let effects = click_row(&mut console, 4);
    assert_eq!(console.main_menu.category(), "extras");
    assert!(
        !effects.iter().any(|e| matches!(e, Effect::MainMenuMap(_))),
        "entering extras does not reload background map"
    );

    // In extras: row 1 is disabled Workshop placeholder
    assert!(!console.main_menu.entries()[1].enabled);
    assert_eq!(console.main_menu.entries()[1].label, "WORKSHOP");
    console.activate_menu(1);
    assert!(console
        .shell
        .panel
        .output
        .iter()
        .any(|line| line.contains("WORKSHOP: not implemented")));

    // In extras: row 0 is HALF-LIFE 2: LOST COAST
    let lc_effects = click_row(&mut console, 0);
    assert_eq!(console.main_menu.category(), "lostcoast");
    assert!(
        lc_effects
            .iter()
            .any(|e| e == &Effect::MainMenuMap(GameMap::new("lostcoast", "background01"))),
        "entering lostcoast submenu loads Lost Coast's own background01"
    );
    assert_eq!(
        console.config.gameplay.background_map.as_deref(),
        Some("background01")
    );
    assert_eq!(
        console.config.gameplay.background_game.as_deref(),
        Some("lostcoast")
    );
    console.level_loaded_for_map("lostcoast", "background01", true);

    // In lostcoast: row 0 is NEW GAME
    let new_game_effects = open_and_start(&mut console);
    assert!(
        new_game_effects
            .iter()
            .any(|e| matches!(e, Effect::ChapterMap(target) if target == &GameMap::new("lostcoast", "d2_lostcoast"))),
        "starts loading d2_lostcoast"
    );
    assert!(console.is_loading());
    assert_eq!(console.loading_dialog.as_ref().unwrap().map, "d2_lostcoast");

    // Simulate missing content error
    console.loading_failed("owned map d2_lostcoast.bsp not found");
    assert!(!console.is_loading());
    assert!(console.loading_dialog.as_ref().unwrap().is_failed());
    assert_eq!(
        console.loading_dialog.as_ref().unwrap().error.as_deref(),
        Some("owned map d2_lostcoast.bsp not found")
    );

    // Dismiss error with Escape -> returns to menu without crash
    console.escape();
    assert!(console.loading_dialog.is_none());
    assert_eq!(console.mode(), Mode::MainMenu);
}

#[test]
fn chapter_cancel_preserves_category_background_and_escape_waits_for_release() {
    let mut console = installed_console();
    console.level_loaded_for_map("ep2", "ep2_background01", true);
    console.activate_menu(2);
    console.open_chapters("ep2");
    let saved = console.background();
    console.escape_pressed();
    assert!(console.chapter_dialog().unwrap().escape_held);
    assert_eq!(console.mode(), Mode::NewGame);
    console.escape_released();
    assert_eq!(console.mode(), Mode::MainMenu);
    assert_eq!(console.menu_category(), "ep2");
    assert_eq!(console.background(), saved);
    assert!(console.chapters_report().is_none());
    assert!(console.loading_dialog.is_none());
}

#[test]
fn chapter_window_remains_interactive_during_background_load() {
    let mut console = installed_console();
    console.level_loaded(true);
    console.activate_menu(1);
    assert!(console.background_transition.is_active());
    console.activate_menu(0);
    assert_eq!(console.mode(), Mode::NewGame);
    let effects = console.input(&Input {
        keys: [Key::Enter].into(),
        ..Default::default()
    });
    assert!(effects.contains(&Effect::ChapterMap(GameMap::new(
        "episodic",
        "ep1_citadel_00"
    ))));
}
