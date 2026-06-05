use std::path::PathBuf;

use clap::Parser;

use crate::app::Tab;
use crate::domain::{ResourceId, ResourceIdError};
use crate::github::api::ApiDepth;
use crate::render::{ContentWidthMode, ScrollbarMode, SpacingMode, SymbolMode, ThemeName};
use crate::session::RestoreMode;

pub const DEFAULT_REFRESH_SECONDS: u64 = 300;

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

    /// Start a new saved restore session for this launch context.
    #[arg(long)]
    pub new: bool,

    /// Disable session restore and avoid binding this launch context.
    #[arg(long)]
    pub no_restore: bool,

    /// Load or create a named restore session.
    #[arg(long, value_name = "ID_OR_NAME")]
    pub session: Option<String>,

    /// Load a normalized resource fixture instead of calling GitHub.
    #[arg(long, value_name = "PATH")]
    pub offline_fixture: Option<PathBuf>,

    /// Additional normalized resource fixture for offline click-through navigation.
    #[arg(long, value_name = "PATH")]
    pub offline_resource_fixture: Vec<PathBuf>,

    /// Disable mouse capture.
    #[arg(long)]
    pub no_mouse: bool,

    /// Refresh interval in seconds.
    #[arg(long, default_value_t = DEFAULT_REFRESH_SECONDS)]
    pub refresh_seconds: u64,

    /// GitHub API depth: partial keeps quota low, full follows all supported pages.
    #[arg(long, value_name = "DEPTH")]
    pub api_depth: Option<ApiDepth>,

    /// Render one frame and exit. Useful for tests and terminal capture.
    #[arg(long)]
    pub once: bool,

    /// Initial tab to show: overview, activity, commits, checks, files, or links.
    #[arg(long, value_name = "TAB")]
    pub tab: Option<Tab>,

    /// Override configured UI theme.
    #[arg(long, value_name = "THEME")]
    pub theme: Option<ThemeName>,

    /// Override configured symbol style: ascii or emoji.
    #[arg(long, value_name = "SYMBOLS")]
    pub symbols: Option<SymbolMode>,

    /// Override configured spacing: comfortable or compact.
    #[arg(long, value_name = "SPACING")]
    pub spacing: Option<SpacingMode>,

    /// Override configured content width mode: fixed or full.
    #[arg(long, value_name = "MODE")]
    pub width_mode: Option<ContentWidthMode>,

    /// Override configured fixed content width.
    #[arg(long, value_name = "COLUMNS")]
    pub fixed_width: Option<u16>,

    /// Override configured scrollbar visibility: always, on-scroll, or hidden.
    #[arg(long, value_name = "MODE")]
    pub scrollbar: Option<ScrollbarMode>,
}

impl Cli {
    pub fn restore_mode(&self) -> RestoreMode {
        if self.no_restore {
            RestoreMode::NoRestore
        } else if self.new {
            RestoreMode::New
        } else {
            RestoreMode::Auto
        }
    }

    pub fn has_resource_arg(&self) -> bool {
        !self.resource.is_empty()
    }

    pub fn parse_resource_id(&self) -> Result<ResourceId, ResourceIdError> {
        match self.resource.as_slice() {
            [single] => ResourceId::parse(single),
            [owner_repo, number] => ResourceId::from_owner_repo_number(owner_repo, number),
            _ => Err(ResourceIdError::Invalid),
        }
    }

    pub fn parse_optional_resource_id(&self) -> Result<Option<ResourceId>, ResourceIdError> {
        if self.resource.is_empty() {
            Ok(None)
        } else {
            self.parse_resource_id().map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_single_resource_arg() {
        let cli = Cli::parse_from(["ghzinga", "openclaw/openclaw#81834"]);

        assert_eq!(
            cli.parse_resource_id().unwrap().canonical_name(),
            "openclaw/openclaw#81834"
        );
    }

    #[test]
    fn parses_restore_flags() {
        let cli = Cli::parse_from([
            "ghzinga",
            "--new",
            "--session",
            "work",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(cli.restore_mode(), RestoreMode::New);
        assert_eq!(cli.session.as_deref(), Some("work"));
        assert!(cli.has_resource_arg());
    }

    #[test]
    fn allows_empty_resource_for_restore() {
        let cli = Cli::parse_from(["ghzinga"]);

        assert_eq!(cli.parse_optional_resource_id().unwrap(), None);
    }

    #[test]
    fn default_refresh_interval_is_conservative() {
        let cli = Cli::parse_from(["ghzinga", "openclaw/openclaw#81834"]);

        assert_eq!(cli.refresh_seconds, DEFAULT_REFRESH_SECONDS);
    }

    #[test]
    fn parses_owner_repo_and_number_args() {
        let cli = Cli::parse_from(["ghzinga", "openclaw/openclaw", "81834"]);

        assert_eq!(
            cli.parse_resource_id().unwrap().canonical_name(),
            "openclaw/openclaw#81834"
        );
    }

    #[test]
    fn parses_offline_fixture_flag() {
        let cli = Cli::parse_from([
            "ghzinga",
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
    fn parses_additional_offline_resource_fixtures() {
        let cli = Cli::parse_from([
            "ghzinga",
            "--offline-fixture",
            "fixtures/pr-81834.json",
            "--offline-resource-fixture",
            "fixtures/issue-66943.json",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(
            cli.offline_resource_fixture,
            [PathBuf::from("fixtures/issue-66943.json")]
        );
    }

    #[test]
    fn parses_initial_tab() {
        let cli = Cli::parse_from(["ghzinga", "--tab", "checks", "openclaw/openclaw#81834"]);

        assert_eq!(cli.tab, Some(Tab::Checks));
    }

    #[test]
    fn parses_api_depth() {
        let cli = Cli::parse_from(["ghzinga", "--api-depth", "full", "openclaw/openclaw#81834"]);

        assert_eq!(cli.api_depth, Some(ApiDepth::Full));
    }

    #[test]
    fn parses_theme() {
        let cli = Cli::parse_from([
            "ghzinga",
            "--theme",
            "solarized-dark",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(cli.theme, Some(ThemeName::Solarized));
    }

    #[test]
    fn parses_symbol_mode() {
        let cli = Cli::parse_from(["ghzinga", "--symbols", "emoji", "openclaw/openclaw#81834"]);

        assert_eq!(cli.symbols, Some(SymbolMode::Emoji));
    }

    #[test]
    fn parses_spacing_mode() {
        let cli = Cli::parse_from(["ghzinga", "--spacing", "compact", "openclaw/openclaw#81834"]);

        assert_eq!(cli.spacing, Some(SpacingMode::Compact));
    }

    #[test]
    fn parses_width_preferences() {
        let cli = Cli::parse_from([
            "ghzinga",
            "--width-mode",
            "full",
            "--fixed-width",
            "132",
            "--scrollbar",
            "always",
            "openclaw/openclaw#81834",
        ]);

        assert_eq!(cli.width_mode, Some(ContentWidthMode::Full));
        assert_eq!(cli.fixed_width, Some(132));
        assert_eq!(cli.scrollbar, Some(ScrollbarMode::Always));
    }
}
