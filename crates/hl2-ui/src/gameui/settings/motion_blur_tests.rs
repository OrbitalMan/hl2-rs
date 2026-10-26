use super::AdvancedChoice;
use crate::{
    config::{AdvancedVideoSettings, Config},
    console::Console,
};

#[test]
fn motion_blur_has_retail_off_on_and_opt_in_advanced() {
    let choice = AdvancedChoice::MotionBlur;
    let mut a = AdvancedVideoSettings::default();
    assert_eq!(choice.selected(&a), 1);
    assert_eq!(
        choice.tokens(),
        [
            "#gameui_disabled",
            "#gameui_enabled",
            "#GameUI_SettingAdvanced"
        ]
    );
    choice.select(&mut a, 2);
    assert!(a.motion_blur && a.motion_blur_viewmodel);
    assert_eq!(choice.selected(&a), 2);
    choice.select(&mut a, 3);
    assert_eq!(choice.selected(&a), 2);
    choice.select(&mut a, 1);
    assert!(a.motion_blur && !a.motion_blur_viewmodel);
    choice.select(&mut a, 0);
    assert!(!a.motion_blur && !a.motion_blur_viewmodel);
}

#[test]
fn old_saves_remain_retail_and_advanced_round_trips() {
    for enabled in [false, true] {
        let bytes = format!(r#"{{"video":{{"advanced":{{"motion_blur":{enabled}}}}}}}"#);
        let config = Config::from_json(bytes.as_bytes()).unwrap();
        assert_eq!(config.video.advanced.motion_blur, enabled);
        assert!(!config.video.advanced.motion_blur_viewmodel);
    }
    let mut config = Config::default();
    AdvancedChoice::MotionBlur.select(&mut config.video.advanced, 2);
    let bytes = serde_json::to_vec(&config).unwrap();
    let restored = Config::from_json(&bytes).unwrap();
    assert_eq!(
        AdvancedChoice::MotionBlur.selected(&restored.video.advanced),
        2
    );
    let mut console = Console::default();
    console.config = restored;
    assert_eq!(
        console.cvar_value("mat_motion_blur_enabled").as_deref(),
        Some("1")
    );
    console.submit("mat_motion_blur_enabled 1");
    assert_eq!(
        AdvancedChoice::MotionBlur.selected(&console.config.video.advanced),
        1
    );
}

#[test]
#[ignore = "requires an owned installed Half-Life 2 copy"]
fn owned_motion_blur_advanced_token() {
    use crate::gameui::resources::GameUiResources;
    let root = source_assets::install::discover().unwrap();
    let vfs = source_assets::vpk::Vfs::mount(&root).unwrap();
    let resources = GameUiResources::load(&vfs);
    let tokens = AdvancedChoice::MotionBlur.tokens();
    assert_eq!(resources.localization.text(tokens[2]), "Advanced");
    assert_eq!(resources.localization.text(tokens[0]), "Disabled");
    assert_eq!(resources.localization.text(tokens[1]), "Enabled");
}
