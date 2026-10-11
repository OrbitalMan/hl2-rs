//! VGUI `.res` control layouts (`resource/OptionsSub*.res`, dialogs) as plain data.
//!
//! Only the keys the GameUI pages use are interpreted: class, position, size, visibility,
//! enabled state, label token, command, image and text alignment. Platform conditionals
//! (`[$OSX]`, `[!$WINDOWS]` ...) are already resolved by `keyvalues::parse_resource`.
//! Positions accept the VGUI forms `N`, `rN` (from the right/bottom edge) and `cN` (from the centre).

use source_assets::keyvalues::{self, Entry};

/// Control classes the GameUI pages draw. Unrecognized classes keep their name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlKind {
    Label,
    Button,
    /// `CheckButton`, `CCvarToggleCheckButton`, `CCvarNegateCheckButton`.
    CheckButton,
    RadioButton,
    ComboBox,
    /// `Slider`, `CCvarSlider`.
    Slider,
    TextEntry,
    Divider,
    ImagePanel,
    /// `ListPanel`, `SectionedListPanel`.
    ListPanel,
    UrlButton,
    Other(String),
}

impl ControlKind {
    pub fn from_class(class: &str) -> Self {
        match class.to_ascii_lowercase().as_str() {
            "label" => Self::Label,
            "button" => Self::Button,
            "checkbutton" | "ccvartogglecheckbutton" | "ccvarnegatecheckbutton" => {
                Self::CheckButton
            }
            "radiobutton" => Self::RadioButton,
            "combobox" => Self::ComboBox,
            "slider" | "ccvarslider" => Self::Slider,
            "textentry" => Self::TextEntry,
            "divider" => Self::Divider,
            "imagepanel" => Self::ImagePanel,
            "listpanel" | "sectionedlistpanel" => Self::ListPanel,
            "urlbutton" => Self::UrlButton,
            _ => Self::Other(class.to_owned()),
        }
    }
}

/// One VGUI coordinate along an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coord {
    Start(f32),
    /// Offset from the far edge (`r10`).
    End(f32),
    /// Offset from the centre (`c-20`).
    Center(f32),
}

impl Coord {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (ctor, number): (fn(f32) -> Self, &str) = match text.as_bytes().first()? {
            b'r' | b'R' => (Self::End, &text[1..]),
            b'c' | b'C' => (Self::Center, &text[1..]),
            _ => (Self::Start, text),
        };
        number
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map(ctor)
    }

    /// Resolves against the parent extent (layout units).
    pub fn resolve(self, parent: f32) -> f32 {
        match self {
            Self::Start(v) => v,
            Self::End(v) => parent - v,
            Self::Center(v) => parent * 0.5 + v,
        }
    }
}

/// Horizontal/vertical label alignment (`textAlignment`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    West,
    Center,
    East,
    NorthWest,
}

impl Align {
    fn parse(text: &str) -> Self {
        match text.to_ascii_lowercase().as_str() {
            "center" => Self::Center,
            "east" => Self::East,
            "north-west" | "northwest" => Self::NorthWest,
            _ => Self::West,
        }
    }
}

/// A control record from a `.res` file, in layout units (pixels at the authored size).
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub name: String,
    pub kind: ControlKind,
    pub x: Coord,
    pub y: Coord,
    pub wide: f32,
    pub tall: f32,
    pub visible: bool,
    pub enabled: bool,
    /// Raw `labelText`, usually a `#Token`.
    pub label: String,
    pub command: String,
    pub image: String,
    pub align: Align,
}

impl Control {
    /// Top-left corner and size inside a parent of `parent` layout units.
    pub fn rect(&self, parent: (f32, f32)) -> (f32, f32, f32, f32) {
        (
            self.x.resolve(parent.0),
            self.y.resolve(parent.1),
            self.wide,
            self.tall,
        )
    }
}

/// All controls of one `.res` file, in file order (VGUI draws siblings in creation order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub controls: Vec<Control>,
}

impl Layout {
    /// Parses `.res` bytes. Returns `None` for unreadable files.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let roots = keyvalues::parse_resource(bytes).ok()?;
        let root = roots.iter().find(|e| !e.children().is_empty())?;
        Some(Self::from_entry(root))
    }

    pub fn from_entry(root: &Entry) -> Self {
        let controls = root
            .children()
            .iter()
            .filter(|e| !e.children().is_empty())
            .filter_map(control)
            .collect();
        Self { controls }
    }

    pub fn get(&self, name: &str) -> Option<&Control> {
        self.controls
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Control> {
        self.controls
            .iter_mut()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

fn control(entry: &Entry) -> Option<Control> {
    let text = |key: &str| entry.get(key).and_then(Entry::text).map(str::trim);
    let class = text("ControlName")?;
    let kind = ControlKind::from_class(class);
    if matches!(&kind, ControlKind::Other(c) if c.eq_ignore_ascii_case("BuildModeDialog")) {
        return None;
    }
    let number = |key: &str| {
        text(key)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.)
            .unwrap_or(0.)
    };
    let flag = |key: &str, default: bool| text(key).map_or(default, |v| v != "0");
    Some(Control {
        name: text("fieldName").unwrap_or(&entry.key).to_owned(),
        kind,
        x: text("xpos")
            .and_then(Coord::parse)
            .unwrap_or(Coord::Start(0.)),
        y: text("ypos")
            .and_then(Coord::parse)
            .unwrap_or(Coord::Start(0.)),
        wide: number("wide"),
        tall: number("tall"),
        visible: flag("visible", true),
        enabled: flag("enabled", true),
        label: text("labelText").unwrap_or_default().to_owned(),
        command: text("command").unwrap_or_default().to_owned(),
        image: text("image").unwrap_or_default().to_owned(),
        align: text("textAlignment").map(Align::parse).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUSE: &str = r##""Resource/OptionsSubMouse.res"
{
    "ReverseMouse" { "ControlName" "CCvarNegateCheckButton" "fieldName" "ReverseMouse"
        "xpos" "28" "ypos" "9" "wide" "150" "tall" "28" "labelText" "#GameUI_ReverseMouse" }
    "MouseRaw" { "ControlName" "CCvarToggleCheckButton" "fieldName" "MouseRaw"
        "xpos" "30" "ypos" "70" "wide" "150" "tall" "24"
        "enabled" "1" [!$OSX]
        "enabled" "0" [$OSX] }
    "BuildModeDialog" { "ControlName" "BuildModeDialog" "xpos" "1081" }
    "Hidden" { "ControlName" "Label" "visible" "0" "xpos" "r40" "ypos" "c-12" "wide" "20" }
    "Slider" { "ControlName" "CCvarSlider" "xpos" "200" "ypos" "40" "wide" "200" "tall" "40" }
}"##;

    #[test]
    fn res_controls_keep_order_classes_conditions_and_coordinates() {
        let layout = Layout::parse(MOUSE.as_bytes()).unwrap();
        let names: Vec<_> = layout.controls.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["ReverseMouse", "MouseRaw", "Hidden", "Slider"],
            "BuildModeDialog skipped"
        );
        let reverse = layout.get("reversemouse").unwrap();
        assert_eq!(reverse.kind, ControlKind::CheckButton);
        assert_eq!(reverse.label, "#GameUI_ReverseMouse");
        assert_eq!(reverse.rect((460., 300.)), (28., 9., 150., 28.));
        assert_eq!(
            layout.get("MouseRaw").unwrap().enabled,
            !cfg!(target_os = "macos")
        );
        let hidden = layout.get("Hidden").unwrap();
        assert!(!hidden.visible);
        assert_eq!(hidden.rect((400., 300.)).0, 360.);
        assert_eq!(hidden.rect((400., 300.)).1, 138.);
        assert_eq!(layout.get("Slider").unwrap().kind, ControlKind::Slider);
    }

    #[test]
    fn coordinates_reject_garbage() {
        assert_eq!(Coord::parse("r12"), Some(Coord::End(12.)));
        assert_eq!(Coord::parse("c-5"), Some(Coord::Center(-5.)));
        assert_eq!(Coord::parse("NaN"), None);
        assert_eq!(Coord::parse(""), None);
        assert!(Layout::parse(b"{ unbalanced").is_none());
    }
}
