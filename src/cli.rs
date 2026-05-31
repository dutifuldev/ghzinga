use std::path::PathBuf;

use clap::Parser;

use crate::app::Tab;
use crate::domain::{ResourceId, ResourceIdError};
use crate::render::ThemeName;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Monitor a GitHub PR or issue in a terminal UI"
)]
pub struct Cli {
    /// GitHub resource as URL, owner/repo#number, or owner/repo number.
    #[arg(value_name = "RESOURCE")]
    pub resource: Vec<String>,

    /// Load a normalized resource fixture instead of calling GitHub.
    #[arg(long, value_name = "PATH")]
    pub offline_fixture: Option<PathBuf>,

    /// Disable mouse capture.
    #[arg(long)]
    pub no_mouse: bool,

    /// Refresh interval in seconds.
    #[arg(long, default_value_t = 60)]
    pub refresh_seconds: u64,

    /// Render one frame and exit. Useful for tests and terminal capture.
    #[arg(long)]
    pub once: bool,

    /// Initial tab to show: overview, activity, commits, checks, files, or links.
    #[arg(long, value_name = "TAB")]
    pub tab: Option<Tab>,

    /// UI theme: default or solarized-dark.
    #[arg(long, default_value_t = ThemeName::Default, value_name = "THEME")]
    pub theme: ThemeName,
}

impl Cli {
    pub fn parse_resource_id(&self) -> Result<ResourceId, ResourceIdError> {
        match self.resource.as_slice() {
            [single] => ResourceId::parse(single),
            [owner_repo, number] => ResourceId::from_owner_repo_number(owner_repo, number),
            _ => Err(ResourceIdError::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_single_resource_arg() {
        let cli = Cli::parse_from(["ghzoom", "openclaw/openclaw#81834"]);

        assert_eq!(
            cli.parse_resource_id().unwrap().canonical_name(),
            "openclaw/openclaw#81834"
        );
    }

    #[test]
    fn parses_owner_repo_and_number_args() {
        let cli = Cli::parse_from(["ghzoom", "openclaw/openclaw", "81834"]);

        assert_eq!(
            cli.parse_resource_id().unwrap().canonical_name(),
            "openclaw/openclaw#81834"
        );
    }

    #[test]
    fn parses_offline_fixture_flag() {
        let cli = Cli::parse_from([
            "ghzoom",
            "--offline-fixture",
            "fixtures/pr-81834.json",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(
            cli.offline_fixture.unwrap(),
            PathBuf::from("fixtures/pr-81834.json")
        );
    }

    #[test]
    fn parses_initial_tab() {
        let cli = Cli::parse_from(["ghzoom", "--tab", "checks", "openclaw/openclaw#81834"]);

        assert_eq!(cli.tab, Some(Tab::Checks));
    }

    #[test]
    fn parses_theme() {
        let cli = Cli::parse_from([
            "ghzoom",
            "--theme",
            "solarized-dark",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(cli.theme, ThemeName::SolarizedDark);
    }
}
