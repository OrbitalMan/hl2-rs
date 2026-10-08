//! Options menu dialog implementing Video, Audio, Mouse and Keyboard configuration.
//! Uses native VGUI styling, layout metrics and localized token lookup from game resources.

use crate::{
    canvas::{Canvas, Color, Rect, WHITE},
    config::{Config, HdrMode},
    console::{rectangle_lines, Key},
    hud::FontFace,
};
use glam::{vec2, Vec2};
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Video,
    Audio,
    Mouse,
    Keyboard,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OptionsResult {
    None,
    Apply(Config),
    Ok(Config),
    Cancel,
}

pub struct OptionsLabels {
    pub title: String,
    pub keyboard: String,
    pub mouse: String,
    pub audio: String,
    pub video: String,
    pub ok: String,
    pub cancel: String,
    pub apply: String,
    pub resolution: String,
    pub display_mode: String,
    pub windowed: String,
    pub fullscreen: String,
    pub fov: String,
    pub hdr: String,
    pub hdr_none: String,
    pub hdr_full: String,
    pub volume: String,
    pub reverse_mouse: String,
    pub sensitivity: String,
}

impl Default for OptionsLabels {
    fn default() -> Self {
        Self {
            title: "OPTIONS".into(),
            keyboard: "Keyboard".into(),
            mouse: "Mouse".into(),
            audio: "Audio".into(),
            video: "Video".into(),
            ok: "OK".into(),
            cancel: "Cancel".into(),
            apply: "Apply".into(),
            resolution: "Resolution".into(),
            display_mode: "Display mode".into(),
            windowed: "Windowed".into(),
            fullscreen: "Borderless Fullscreen".into(),
            fov: "Field of view".into(),
            hdr: "High Dynamic Range".into(),
            hdr_none: "None".into(),
            hdr_full: "Full (if available)".into(),
            volume: "Game volume".into(),
            reverse_mouse: "Reverse mouse".into(),
            sensitivity: "Mouse sensitivity".into(),
        }
    }
}

impl OptionsLabels {
    pub fn load(vfs: &Vfs) -> Self {
        let mut labels = Self::default();
        if let Some(root) = vfs
            .read("resource/gameui_english.txt")
            .ok()
            .flatten()
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            if let Some(tokens) = root.get("Tokens") {
                let lookup = |key: &str| -> Option<String> {
                    tokens.get(key).and_then(|e| e.text()).map(str::to_owned)
                };
                if let Some(v) = lookup("GameUI_Options") {
                    labels.title = v.to_uppercase();
                }
                if let Some(v) = lookup("GameUI_Keyboard") {
                    labels.keyboard = v;
                }
                if let Some(v) = lookup("GameUI_Mouse") {
                    labels.mouse = v;
                }
                if let Some(v) = lookup("GameUI_Audio") {
                    labels.audio = v;
                }
                if let Some(v) = lookup("GameUI_Video") {
                    labels.video = v;
                }
                if let Some(v) = lookup("GameUI_OK") {
                    labels.ok = v;
                }
                if let Some(v) = lookup("GameUI_Cancel") {
                    labels.cancel = v;
                }
                if let Some(v) = lookup("GameUI_Apply") {
                    labels.apply = v;
                }
                if let Some(v) = lookup("GameUI_Resolution") {
                    labels.resolution = v;
                }
                if let Some(v) = lookup("GameUI_DisplayMode") {
                    labels.display_mode = v;
                }
                if let Some(v) = lookup("GameUI_Windowed") {
                    labels.windowed = v;
                }
                if let Some(v) = lookup("GameUI_Fullscreen") {
                    labels.fullscreen = v;
                }
                if let Some(v) = lookup("GameUI_FOV") {
                    labels.fov = v;
                }
                if let Some(v) = lookup("GameUI_HDR") {
                    labels.hdr = v;
                }
                if let Some(v) = lookup("GameUI_hdr_level0") {
                    labels.hdr_none = v;
                }
                if let Some(v) = lookup("GameUI_hdr_level2") {
                    labels.hdr_full = v;
                }
                if let Some(v) = lookup("GameUI_SoundEffectVolume") {
                    labels.volume = v;
                }
                if let Some(v) = lookup("GameUI_ReverseMouse") {
                    labels.reverse_mouse = v;
                }
                if let Some(v) = lookup("GameUI_MouseSensitivity") {
                    labels.sensitivity = v;
                }
            }
        }
        labels
    }
}

pub const KEYBOARD_ACTIONS: &[(&str, &str, &str)] = &[
    ("+forward", "Move forward", "Valve_Move_Forward"),
    ("+back", "Move back", "Valve_Move_Back"),
    ("+moveleft", "Move left", "Valve_Move_Left"),
    ("+moveright", "Move right", "Valve_Move_Right"),
    ("+jump", "Jump", "Valve_Jump"),
    ("+duck", "Duck", "Valve_Duck"),
    ("+speed", "Sprint", "Valve_Sprint"),
    ("+walk", "Walk", "Valve_Walk"),
    ("+use", "Use items", "Valve_Use_Items"),
    ("+reload", "Reload weapon", "Valve_Reload_Weapon"),
    ("lastinv", "Last weapon used", "Valve_Last_Weapon_Used"),
    ("slot1", "Weapon category 1", "Valve_Weapon_Category_1"),
    ("slot2", "Weapon category 2", "Valve_Weapon_Category_2"),
    ("slot3", "Weapon category 3", "Valve_Weapon_Category_3"),
    ("slot4", "Weapon category 4", "Valve_Weapon_Category_4"),
    ("slot5", "Weapon category 5", "Valve_Weapon_Category_5"),
    ("slot6", "Weapon category 6", "Valve_Weapon_Category_6"),
];

pub struct OptionsState {
    pub tab: OptionsTab,
    pub draft: Config,
    pub original: Config,
    pub labels: OptionsLabels,
    pub action_labels: BTreeMap<String, String>,
    pub rebinding: Option<String>,
    pub scroll: usize,
    pub dragging_slider: Option<&'static str>,
}

impl Default for OptionsState {
    fn default() -> Self {
        Self::new(Config::default(), OptionsLabels::default(), BTreeMap::new())
    }
}

impl OptionsState {
    pub fn new(
        config: Config,
        labels: OptionsLabels,
        action_labels: BTreeMap<String, String>,
    ) -> Self {
        Self {
            tab: OptionsTab::Video,
            draft: config.clone(),
            original: config,
            labels,
            action_labels,
            rebinding: None,
            scroll: 0,
            dragging_slider: None,
        }
    }

    pub fn load(vfs: &Vfs, config: Config) -> Self {
        let labels = OptionsLabels::load(vfs);
        let mut action_labels = BTreeMap::new();
        if let Some(root) = vfs
            .read("resource/valve_english.txt")
            .ok()
            .flatten()
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            if let Some(tokens) = root.get("Tokens") {
                for (action, default_name, token_key) in KEYBOARD_ACTIONS {
                    let text = tokens
                        .get(token_key)
                        .and_then(|e| e.text())
                        .unwrap_or(default_name);
                    action_labels.insert((*action).to_owned(), text.to_owned());
                }
            }
        }
        for (action, default_name, _) in KEYBOARD_ACTIONS {
            action_labels
                .entry((*action).to_owned())
                .or_insert_with(|| (*default_name).to_owned());
        }
        Self::new(config, labels, action_labels)
    }

    pub fn reset(&mut self, config: Config) {
        self.draft = config.clone();
        self.original = config;
        self.rebinding = None;
        self.dragging_slider = None;
    }

    pub fn input(&mut self, input: &crate::console::Input, canvas_size: Vec2) -> OptionsResult {
        let scale = canvas_size.y / 720.0;
        let w = (580.0 * scale).min(canvas_size.x - 40.0);
        let h = (460.0 * scale).min(canvas_size.y - 40.0);
        let left = ((canvas_size.x - w) * 0.5).trunc();
        let top = ((canvas_size.y - h) * 0.5).trunc();

        // Handle rebinding in Keyboard tab
        if let Some(action) = self.rebinding.take() {
            let bound_key = input.keys.iter().next().map(|key| match key {
                Key::Up => "ArrowUp",
                Key::Down => "ArrowDown",
                Key::Left => "ArrowLeft",
                Key::Right => "ArrowRight",
                Key::Enter => "Enter",
                Key::Backspace => "Backspace",
                Key::Delete => "Delete",
                Key::Home => "Home",
                Key::End => "End",
                Key::PageUp => "PageUp",
                Key::PageDown => "PageDown",
                Key::Tab => "Tab",
            });
            if bound_key.is_none() && !input.text.is_empty() {
                let first_char = input.text.chars().next().unwrap();
                if first_char.is_ascii_alphabetic() {
                    let k = format!("Key{}", first_char.to_ascii_uppercase());
                    self.draft.keys.insert(action.clone(), k);
                    return OptionsResult::None;
                } else if first_char.is_ascii_digit() {
                    let k = format!("Digit{}", first_char);
                    self.draft.keys.insert(action.clone(), k);
                    return OptionsResult::None;
                } else if first_char == ' ' {
                    self.draft.keys.insert(action.clone(), "Space".into());
                    return OptionsResult::None;
                }
            }
            if let Some(name) = bound_key {
                self.draft.keys.insert(action, name.to_owned());
                return OptionsResult::None;
            }
            // Put it back if no key was captured
            self.rebinding = Some(action);
        }

        let Some(p) = input.pointer else {
            return OptionsResult::None;
        };

        let btn_w = 80.0 * scale;
        let btn_h = 24.0 * scale;
        let btn_y = top + h - 34.0 * scale;

        // Bottom buttons
        let ok_rect = Rect::new(left + w - 3.0 * (btn_w + 10.0 * scale), btn_y, btn_w, btn_h);
        let cancel_rect = Rect::new(left + w - 2.0 * (btn_w + 10.0 * scale), btn_y, btn_w, btn_h);
        let apply_rect = Rect::new(left + w - (btn_w + 10.0 * scale), btn_y, btn_w, btn_h);

        if input.click {
            if ok_rect.contains(p) {
                return OptionsResult::Ok(self.draft.clone());
            }
            if cancel_rect.contains(p) {
                self.draft = self.original.clone();
                return OptionsResult::Cancel;
            }
            if apply_rect.contains(p) {
                self.original = self.draft.clone();
                return OptionsResult::Apply(self.draft.clone());
            }

            // Tabs
            let tab_w = 90.0 * scale;
            let tab_h = 24.0 * scale;
            let tab_y = top + 26.0 * scale;
            let tabs = [
                (OptionsTab::Keyboard, 0.0),
                (OptionsTab::Mouse, 1.0),
                (OptionsTab::Audio, 2.0),
                (OptionsTab::Video, 3.0),
            ];
            for (tab, idx) in tabs {
                let rect = Rect::new(
                    left + 16.0 * scale + idx * (tab_w + 4.0 * scale),
                    tab_y,
                    tab_w,
                    tab_h,
                );
                if rect.contains(p) {
                    self.tab = tab;
                    self.rebinding = None;
                    return OptionsResult::None;
                }
            }
        }

        // Tab Content interactions
        let content_left = left + 16.0 * scale;
        let content_top = top + 54.0 * scale;

        match self.tab {
            OptionsTab::Video => {
                // Resolution Selector
                let res_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 20.0 * scale,
                    180.0 * scale,
                    24.0 * scale,
                );
                if input.click && res_rect.contains(p) {
                    let common_resolutions =
                        [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)];
                    let current = (self.draft.video.width, self.draft.video.height);
                    let next_idx = common_resolutions
                        .iter()
                        .position(|&r| r == current)
                        .map(|i| (i + 1) % common_resolutions.len())
                        .unwrap_or(0);
                    let (w, h) = common_resolutions[next_idx];
                    self.draft.video.width = w;
                    self.draft.video.height = h;
                }

                // Display Mode Toggle
                let mode_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 55.0 * scale,
                    180.0 * scale,
                    24.0 * scale,
                );
                if input.click && mode_rect.contains(p) {
                    self.draft.video.borderless = !self.draft.video.borderless;
                }

                // FOV Slider
                let fov_slider_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 90.0 * scale,
                    180.0 * scale,
                    20.0 * scale,
                );
                if (input.click || self.dragging_slider == Some("fov"))
                    && fov_slider_rect.contains(p)
                {
                    let frac = ((p.x - fov_slider_rect.x) / fov_slider_rect.w).clamp(0.0, 1.0);
                    self.draft.video.fov = (75.0 + frac * (110.0 - 75.0)).round();
                    self.dragging_slider = Some("fov");
                } else if !input.click {
                    self.dragging_slider = None;
                }

                // HDR Mode Toggle (None / Full)
                let hdr_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 125.0 * scale,
                    180.0 * scale,
                    24.0 * scale,
                );
                if input.click && hdr_rect.contains(p) {
                    self.draft.video.hdr = match self.draft.video.hdr {
                        HdrMode::None => HdrMode::Full,
                        HdrMode::Full => HdrMode::None,
                    };
                }
            }
            OptionsTab::Audio => {
                // Volume Slider
                let vol_slider_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 20.0 * scale,
                    180.0 * scale,
                    20.0 * scale,
                );
                if (input.click || self.dragging_slider == Some("volume"))
                    && vol_slider_rect.contains(p)
                {
                    let frac = ((p.x - vol_slider_rect.x) / vol_slider_rect.w).clamp(0.0, 1.0);
                    self.draft.audio.volume = (frac * 100.0).round() / 100.0;
                    self.dragging_slider = Some("volume");
                } else if !input.click {
                    self.dragging_slider = None;
                }
            }
            OptionsTab::Mouse => {
                // Reverse Mouse Checkbox
                let rev_rect = Rect::new(
                    content_left + 20.0 * scale,
                    content_top + 20.0 * scale,
                    200.0 * scale,
                    24.0 * scale,
                );
                if input.click && rev_rect.contains(p) {
                    self.draft.mouse.invert = !self.draft.mouse.invert;
                }

                // Sensitivity Slider
                let sens_slider_rect = Rect::new(
                    content_left + 150.0 * scale,
                    content_top + 55.0 * scale,
                    180.0 * scale,
                    20.0 * scale,
                );
                if (input.click || self.dragging_slider == Some("sens"))
                    && sens_slider_rect.contains(p)
                {
                    let frac = ((p.x - sens_slider_rect.x) / sens_slider_rect.w).clamp(0.0, 1.0);
                    self.draft.mouse.sensitivity = ((0.5 + frac * 9.5) * 10.0).round() / 10.0;
                    self.dragging_slider = Some("sens");
                } else if !input.click {
                    self.dragging_slider = None;
                }
            }
            OptionsTab::Keyboard => {
                // Actions list
                let row_h = 22.0 * scale;
                let visible_rows = 12;
                for i in 0..visible_rows {
                    let action_idx = self.scroll + i;
                    if action_idx >= KEYBOARD_ACTIONS.len() {
                        break;
                    }
                    let (action, _, _) = KEYBOARD_ACTIONS[action_idx];
                    let row_y = content_top + 10.0 * scale + i as f32 * row_h;
                    let row_rect =
                        Rect::new(content_left + 10.0 * scale, row_y, w - 52.0 * scale, row_h);
                    if input.click && row_rect.contains(p) {
                        self.rebinding = Some(action.to_string());
                        break;
                    }
                }
                if input.pressed(Key::PageDown) {
                    self.scroll =
                        (self.scroll + 4).min(KEYBOARD_ACTIONS.len().saturating_sub(visible_rows));
                }
                if input.pressed(Key::PageUp) {
                    self.scroll = self.scroll.saturating_sub(4);
                }
            }
        }

        OptionsResult::None
    }

    pub(crate) fn draw(
        &self,
        canvas: &Canvas,
        font: Option<&FontFace>,
        menu_font: Option<&FontFace>,
    ) {
        let canvas_size = vec2(canvas.width(), canvas.height());
        let scale = canvas_size.y / 720.0;
        let w = (580.0 * scale).min(canvas_size.x - 40.0);
        let h = (460.0 * scale).min(canvas_size.y - 40.0);
        let left = ((canvas_size.x - w) * 0.5).trunc();
        let top = ((canvas_size.y - h) * 0.5).trunc();

        let panel_bg = Color::from_rgba(65, 68, 60, 240);
        let entry_bg = Color::from_rgba(30, 32, 28, 230);
        let border_col = Color::from_rgba(150, 158, 147, 255);
        let text_col = Color::from_rgba(221, 221, 221, 255);
        let active_tab_bg = Color::from_rgba(95, 100, 90, 255);
        let inactive_tab_bg = Color::from_rgba(50, 52, 45, 255);

        // Dark modal scrim
        canvas.rectangle(
            0.,
            0.,
            canvas.width(),
            canvas.height(),
            Color::new(0., 0., 0., 0.4),
        );

        // Dialog frame
        canvas.rectangle(left, top, w, h, panel_bg);
        rectangle_lines(canvas, left, top, w, h, 1.0, border_col);

        // Dialog title
        if let Some(f) = font {
            f.draw_baseline(
                &self.labels.title,
                vec2(left + 16.0 * scale, top + 18.0 * scale),
                16.0 * scale,
                1.0,
                WHITE,
            );
        }

        // Tabs
        let tab_w = 90.0 * scale;
        let tab_h = 24.0 * scale;
        let tab_y = top + 26.0 * scale;
        let tabs = [
            (OptionsTab::Keyboard, &self.labels.keyboard, 0.0),
            (OptionsTab::Mouse, &self.labels.mouse, 1.0),
            (OptionsTab::Audio, &self.labels.audio, 2.0),
            (OptionsTab::Video, &self.labels.video, 3.0),
        ];

        for (tab, label, idx) in tabs {
            let tx = left + 16.0 * scale + idx * (tab_w + 4.0 * scale);
            let bg = if self.tab == tab {
                active_tab_bg
            } else {
                inactive_tab_bg
            };
            canvas.rectangle(tx, tab_y, tab_w, tab_h, bg);
            rectangle_lines(canvas, tx, tab_y, tab_w, tab_h, 1.0, border_col);
            if let Some(f) = font {
                f.draw_baseline(
                    label,
                    vec2(tx + 8.0 * scale, tab_y + 16.0 * scale),
                    14.0 * scale,
                    1.0,
                    WHITE,
                );
            }
        }

        // Content container
        let content_left = left + 16.0 * scale;
        let content_top = top + 54.0 * scale;
        let content_w = w - 32.0 * scale;
        let content_h = h - 100.0 * scale;
        canvas.rectangle(content_left, content_top, content_w, content_h, entry_bg);
        rectangle_lines(
            canvas,
            content_left,
            content_top,
            content_w,
            content_h,
            1.0,
            border_col,
        );

        // Render controls based on tab
        match self.tab {
            OptionsTab::Video => {
                let lbl_x = content_left + 20.0 * scale;
                let val_x = content_left + 150.0 * scale;

                // Resolution
                let y1 = content_top + 36.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.resolution,
                        vec2(lbl_x, y1),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let res_box = Rect::new(val_x, y1 - 16.0 * scale, 180.0 * scale, 24.0 * scale);
                canvas.rectangle(res_box.x, res_box.y, res_box.w, res_box.h, panel_bg);
                rectangle_lines(
                    canvas, res_box.x, res_box.y, res_box.w, res_box.h, 1.0, border_col,
                );
                let res_str = format!("{} x {}", self.draft.video.width, self.draft.video.height);
                if let Some(f) = font {
                    f.draw_baseline(
                        &res_str,
                        vec2(res_box.x + 8.0 * scale, y1),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }

                // Display mode
                let y2 = y1 + 35.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.display_mode,
                        vec2(lbl_x, y2),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let mode_box = Rect::new(val_x, y2 - 16.0 * scale, 180.0 * scale, 24.0 * scale);
                canvas.rectangle(mode_box.x, mode_box.y, mode_box.w, mode_box.h, panel_bg);
                rectangle_lines(
                    canvas, mode_box.x, mode_box.y, mode_box.w, mode_box.h, 1.0, border_col,
                );
                let mode_str = if self.draft.video.borderless {
                    &self.labels.fullscreen
                } else {
                    &self.labels.windowed
                };
                if let Some(f) = font {
                    f.draw_baseline(
                        mode_str,
                        vec2(mode_box.x + 8.0 * scale, y2),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }

                // FOV Slider
                let y3 = y2 + 35.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.fov,
                        vec2(lbl_x, y3),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let fov_slider = Rect::new(val_x, y3 - 14.0 * scale, 180.0 * scale, 18.0 * scale);
                canvas.rectangle(
                    fov_slider.x,
                    fov_slider.y + 7.0 * scale,
                    fov_slider.w,
                    4.0 * scale,
                    panel_bg,
                );
                let fov_frac = ((self.draft.video.fov - 75.0) / (110.0 - 75.0)).clamp(0.0, 1.0);
                let knob_x = fov_slider.x + fov_frac * (fov_slider.w - 12.0 * scale);
                canvas.rectangle(knob_x, fov_slider.y, 12.0 * scale, fov_slider.h, WHITE);
                let fov_str = format!("{:.0}°", self.draft.video.fov);
                if let Some(f) = font {
                    f.draw_baseline(
                        &fov_str,
                        vec2(val_x + 190.0 * scale, y3),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }

                // HDR Option
                let y4 = y3 + 35.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.hdr,
                        vec2(lbl_x, y4),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let hdr_box = Rect::new(val_x, y4 - 16.0 * scale, 180.0 * scale, 24.0 * scale);
                canvas.rectangle(hdr_box.x, hdr_box.y, hdr_box.w, hdr_box.h, panel_bg);
                rectangle_lines(
                    canvas, hdr_box.x, hdr_box.y, hdr_box.w, hdr_box.h, 1.0, border_col,
                );
                let hdr_str = match self.draft.video.hdr {
                    HdrMode::None => &self.labels.hdr_none,
                    HdrMode::Full => &self.labels.hdr_full,
                };
                if let Some(f) = font {
                    f.draw_baseline(
                        hdr_str,
                        vec2(hdr_box.x + 8.0 * scale, y4),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }
            }
            OptionsTab::Audio => {
                let lbl_x = content_left + 20.0 * scale;
                let val_x = content_left + 150.0 * scale;
                let y1 = content_top + 36.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.volume,
                        vec2(lbl_x, y1),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let vol_slider = Rect::new(val_x, y1 - 14.0 * scale, 180.0 * scale, 18.0 * scale);
                canvas.rectangle(
                    vol_slider.x,
                    vol_slider.y + 7.0 * scale,
                    vol_slider.w,
                    4.0 * scale,
                    panel_bg,
                );
                let vol_frac = self.draft.audio.volume.clamp(0.0, 1.0);
                let knob_x = vol_slider.x + vol_frac * (vol_slider.w - 12.0 * scale);
                canvas.rectangle(knob_x, vol_slider.y, 12.0 * scale, vol_slider.h, WHITE);
                let vol_str = format!("{:.0}%", self.draft.audio.volume * 100.0);
                if let Some(f) = font {
                    f.draw_baseline(
                        &vol_str,
                        vec2(val_x + 190.0 * scale, y1),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }
            }
            OptionsTab::Mouse => {
                let lbl_x = content_left + 20.0 * scale;
                let val_x = content_left + 150.0 * scale;

                // Reverse mouse checkbox
                let y1 = content_top + 36.0 * scale;
                let chk_rect = Rect::new(lbl_x, y1 - 14.0 * scale, 16.0 * scale, 16.0 * scale);
                canvas.rectangle(chk_rect.x, chk_rect.y, chk_rect.w, chk_rect.h, panel_bg);
                rectangle_lines(
                    canvas, chk_rect.x, chk_rect.y, chk_rect.w, chk_rect.h, 1.0, border_col,
                );
                if self.draft.mouse.invert {
                    canvas.rectangle(
                        chk_rect.x + 3.0 * scale,
                        chk_rect.y + 3.0 * scale,
                        chk_rect.w - 6.0 * scale,
                        chk_rect.h - 6.0 * scale,
                        WHITE,
                    );
                }
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.reverse_mouse,
                        vec2(lbl_x + 24.0 * scale, y1),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }

                // Sensitivity slider
                let y2 = y1 + 35.0 * scale;
                if let Some(f) = font {
                    f.draw_baseline(
                        &self.labels.sensitivity,
                        vec2(lbl_x, y2),
                        14.0 * scale,
                        1.0,
                        text_col,
                    );
                }
                let sens_slider = Rect::new(val_x, y2 - 14.0 * scale, 180.0 * scale, 18.0 * scale);
                canvas.rectangle(
                    sens_slider.x,
                    sens_slider.y + 7.0 * scale,
                    sens_slider.w,
                    4.0 * scale,
                    panel_bg,
                );
                let sens_frac = ((self.draft.mouse.sensitivity - 0.5) / 9.5).clamp(0.0, 1.0);
                let knob_x = sens_slider.x + sens_frac * (sens_slider.w - 12.0 * scale);
                canvas.rectangle(knob_x, sens_slider.y, 12.0 * scale, sens_slider.h, WHITE);
                let sens_str = format!("{:.1}", self.draft.mouse.sensitivity);
                if let Some(f) = font {
                    f.draw_baseline(
                        &sens_str,
                        vec2(val_x + 190.0 * scale, y2),
                        14.0 * scale,
                        1.0,
                        WHITE,
                    );
                }
            }
            OptionsTab::Keyboard => {
                let row_h = 22.0 * scale;
                let visible_rows = 12;
                for i in 0..visible_rows {
                    let action_idx = self.scroll + i;
                    if action_idx >= KEYBOARD_ACTIONS.len() {
                        break;
                    }
                    let (action, default_name, _) = KEYBOARD_ACTIONS[action_idx];
                    let row_y = content_top + 10.0 * scale + i as f32 * row_h;
                    let is_active = self.rebinding.as_deref() == Some(action);

                    let bg = if is_active {
                        active_tab_bg
                    } else if i % 2 == 0 {
                        entry_bg
                    } else {
                        panel_bg
                    };
                    canvas.rectangle(
                        content_left + 8.0 * scale,
                        row_y,
                        content_w - 16.0 * scale,
                        row_h,
                        bg,
                    );

                    let display_action = self
                        .action_labels
                        .get(action)
                        .map(String::as_str)
                        .unwrap_or(default_name);
                    let bound_key = self
                        .draft
                        .keys
                        .get(action)
                        .map(String::as_str)
                        .unwrap_or("None");

                    if let Some(f) = font {
                        f.draw_baseline(
                            display_action,
                            vec2(content_left + 16.0 * scale, row_y + 16.0 * scale),
                            13.0 * scale,
                            1.0,
                            text_col,
                        );
                        let key_text = if is_active {
                            "[Press a key...]"
                        } else {
                            bound_key
                        };
                        f.draw_baseline(
                            key_text,
                            vec2(
                                content_left + content_w - 160.0 * scale,
                                row_y + 16.0 * scale,
                            ),
                            13.0 * scale,
                            1.0,
                            if is_active {
                                WHITE
                            } else {
                                Color::from_rgba(240, 200, 120, 255)
                            },
                        );
                    }
                }
            }
        }

        // Bottom action buttons: OK, Cancel, Apply
        let btn_w = 80.0 * scale;
        let btn_h = 24.0 * scale;
        let btn_y = top + h - 34.0 * scale;

        let ok_x = left + w - 3.0 * (btn_w + 10.0 * scale);
        let cancel_x = left + w - 2.0 * (btn_w + 10.0 * scale);
        let apply_x = left + w - (btn_w + 10.0 * scale);

        for (bx, text) in [
            (ok_x, &self.labels.ok),
            (cancel_x, &self.labels.cancel),
            (apply_x, &self.labels.apply),
        ] {
            canvas.rectangle(bx, btn_y, btn_w, btn_h, panel_bg);
            rectangle_lines(canvas, bx, btn_y, btn_w, btn_h, 1.0, border_col);
            if let Some(f) = menu_font.or(font) {
                f.draw_baseline(
                    text,
                    vec2(bx + 14.0 * scale, btn_y + 17.0 * scale),
                    13.0 * scale,
                    1.0,
                    WHITE,
                );
            }
        }
    }
}
