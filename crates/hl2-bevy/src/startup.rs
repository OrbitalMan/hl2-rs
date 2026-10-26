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
