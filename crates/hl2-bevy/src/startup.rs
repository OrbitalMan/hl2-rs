//! Installation/output side effects after argument parsing has succeeded.
use super::Options;
use anyhow::Result;
use std::path::PathBuf;

pub(super) struct InstallationRequest {
    pub explicit: Option<PathBuf>,
    pub force: bool,
    pub noninteractive: bool,
    pub preview: Option<PathBuf>,
}

impl Options {
    pub(super) fn finish_startup(mut self, installation: InstallationRequest) -> Result<Self> {
        self.game = match crate::setup::resolve(
            installation.explicit,
            self.map.as_deref(),
            installation.force,
            installation.noninteractive,
            installation.preview,
        )? {
            Some(path) => path,
            None => std::process::exit(0), // Setup cancelled; no game was loaded.
        };
        for path in [
            Some(&self.report),
            self.capture.as_ref(),
            self.monitor_capture.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(self)
    }
}

const HELP_LINES: &[&str] = &[
    "Usage: hl2-bevy [OPTIONS]\n",
    "Options:",
    "      --ai                            Enable NPC AI think loop (default: enabled)",
    "      --audio-trace <path.jsonl>      Record audio cue and soundscape events to a JSONL trace file",
    "      --background                    Run headlessly without opening an OS window (for background replay / test runs)",
    "      --background-map <name>         Override the 3D background map loaded behind the main menu (default: background01)",
    "      --borderless                    Start in borderless windowed mode (native full-screen Space on macOS)",
    "      --capture <path.png>            Save screenshot of the final rendered frame to a PNG file",
    "      --capture-burst <count>         Capture a sequence of N frames to PNG files (requires --map)",
    "      --capture-live                  Capture frames live during interactive execution (requires --map)",
    "      --capture-monitor <path.png>    Capture an in-game camera monitor texture to a PNG file (requires --map)",
    "      --capture-window                Capture the presented OS window buffer rather than the internal game viewport",
    "      --cpu-skinning                  Use CPU vertex skinning instead of GPU compute skinning",
    "      --fake-monitor <spec>           Simulate a monitor with specified modes (e.g. 1920x1080,1280x720)",
    "      --fly                           Start with flight/noclip mode enabled (requires --map)",
    "      --frag-swept-box                Use swept-box physics query for frag grenade collision",
    "      --frames <count>                Exit automatically after rendering N frames (30..=36000)",
    "      --fullscreen                    Start in exclusive fullscreen display mode",
    "      --game <path>                   Path to Half-Life 2 installation (overrides saved preference and HL2_ROOT)",
    "      --height <pixels>               Window height in pixels (240..=8192, default from video config)",
    "  -h, --help                          Show this help message and exit",
    "      --main-menu                     Open the main menu (overlaid on top of --map if specified)",
    "      --map <name>                    Map to load directly into gameplay (e.g. d1_trainstation_01)",
    "      --max-edicts <count>            Runtime entity limit (default: 2048, 0 for unlimited)",
    "      --movement-script <path.json>   Replay recorded player inputs from a movement script fixture (requires --map)",
    "      --mute                          Silence all audio output for this run (Audio settings unchanged)",
    "      --mute-ambient                  Mute looping ambient soundscapes and background environment audio",
    "      --no-ai                         Disable NPC AI think loop",
    "      --no-focus                      Launch window without requesting input focus (for unattended test runs)",
    "      --novid, -novid                 Skip startup videos on launch",
    "      --no-pvs                        Disable BSP potential visible set (PVS) occlusion culling",
    "      --no-setup                      Fail with an error instead of opening setup if installation is missing",
    "      --pitch <degrees>               Set initial camera pitch angle in degrees (requires --map)",
    "      --position <x> <y> <z>          Set initial camera spawn coordinates (requires --map)",
    "      --profile                       Record CPU stage execution timings in the run report",
    "      --report <path.json>            Path for simulation report output (default: artifacts/bevy-report.json)",
    "      --setup                         Open the setup dialog to choose/remember the Half-Life 2 folder",
    "      --setup-preview <path.png>      Capture a screenshot of the setup dialog and exit without loading a map",
    "      --tonemap-scale <scale>         Override HDR tonemapping exposure multiplier (positive float)",
    "      --uncapped                      Disable VSync to render and present frames unconstrained",
    "      --volume <0.0..1.0>             Set master audio volume level (default from audio config)",
    "      --width <pixels>                Window width in pixels (320..=8192, default from video config)",
    "      --window-at <x> <y>             Initial window position on desktop in pixels",
    "      --windowed                      Start in windowed display mode",
    "      --world-partition               Enable spatial world partition culling",
    "      --yaw <degrees>                 Set initial camera yaw angle in degrees (requires --map)",
];

pub(super) fn print_help() {
    for line in HELP_LINES {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_defers_installation_resolution_and_setup_requests() {
        let args = [
            "--game",
            "not-an-install",
            "--setup",
            "--report",
            "output/report.json",
        ];
        let (options, request) = Options::parse_from(args.map(str::to_owned), Default::default())
            .expect("parsing must not inspect or remember an installation or open setup");
        assert!(options.game.as_os_str().is_empty());
        assert_eq!(request.explicit, Some(PathBuf::from("not-an-install")));
        assert!(request.force);
        assert!(!request.noninteractive);
        assert_eq!(options.report, PathBuf::from("output/report.json"));
    }

    #[test]
    fn parsing_uses_supplied_settings_and_preserves_noninteractive_setup_policy() {
        let mut config = hl2_ui::config::Config::default();
        config.video.width = 1234;
        config.video.height = 777;
        for flag in ["--no-setup", "--background", "--no-focus", "--capture"] {
            let mut args = vec![flag.to_owned()];
            if flag == "--capture" {
                args.push("capture.png".into());
            }
            let (options, request) = Options::parse_from(args, config.clone()).unwrap();
            assert_eq!((options.width, options.height), (1234, 777));
            assert!(request.noninteractive, "{flag}");
            assert!(!request.force);
        }
        let args = ["--setup", "--no-setup"].map(str::to_owned);
        assert!(Options::parse_from(args, config).is_err());
    }
}
