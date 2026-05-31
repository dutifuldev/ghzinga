use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolMode {
    #[default]
    Ascii,
    Emoji,
}

impl SymbolMode {
    pub fn symbols(self) -> Symbols {
        match self {
            Self::Ascii => Symbols::ascii(),
            Self::Emoji => Symbols::emoji(),
        }
    }
}

impl fmt::Display for SymbolMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ascii => f.write_str("ascii"),
            Self::Emoji => f.write_str("emoji"),
        }
    }
}

impl FromStr for SymbolMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "ascii" | "plain" => Ok(Self::Ascii),
            "emoji" => Ok(Self::Emoji),
            _ => Err("expected one of ascii, emoji".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbols {
    pub state_open: &'static str,
    pub state_merged: &'static str,
    pub state_closed: &'static str,
    pub state_unknown: &'static str,
    pub checks_pass: &'static str,
    pub checks_fail: &'static str,
    pub checks_pending: &'static str,
    pub checks_unknown: &'static str,
    pub check_success: &'static str,
    pub check_failure: &'static str,
    pub check_pending: &'static str,
    pub check_skipped: &'static str,
    pub check_neutral: &'static str,
    pub check_unknown: &'static str,
    pub author: &'static str,
    pub comments: &'static str,
    pub reactions: &'static str,
    pub assignees: &'static str,
    pub threads: &'static str,
    pub files: &'static str,
    pub warning: &'static str,
    pub refresh: &'static str,
    pub changed: &'static str,
    pub error: &'static str,
    pub info: &'static str,
    pub body: &'static str,
    pub activity_comment: &'static str,
    pub activity_review: &'static str,
    pub activity_review_comment: &'static str,
    pub activity_commit_comment: &'static str,
    pub activity_timeline: &'static str,
    pub more: &'static str,
    pub less: &'static str,
    pub expand_all: &'static str,
    pub collapse_all: &'static str,
    pub more_patch: &'static str,
    pub less_patch: &'static str,
    pub footer_refresh: &'static str,
    pub footer_open: &'static str,
    pub footer_settings: &'static str,
    pub footer_help: &'static str,
    pub footer_quit: &'static str,
}

impl Symbols {
    fn ascii() -> Self {
        Self {
            state_open: "OK",
            state_merged: "MG",
            state_closed: "XX",
            state_unknown: "--",
            checks_pass: "OK",
            checks_fail: "!!",
            checks_pending: "..",
            checks_unknown: "--",
            check_success: "OK",
            check_failure: "!!",
            check_pending: "..",
            check_skipped: "SK",
            check_neutral: "--",
            check_unknown: "?",
            author: "by",
            comments: "comments",
            reactions: "reactions",
            assignees: "assignees",
            threads: "threads",
            files: "files",
            warning: "warn",
            refresh: "refresh",
            changed: "changed",
            error: "error",
            info: "info",
            body: "*",
            activity_comment: "comment",
            activity_review: "review",
            activity_review_comment: "thread",
            activity_commit_comment: "commit",
            activity_timeline: "-",
            more: "[+ more]",
            less: "[- less]",
            expand_all: "[expand all]",
            collapse_all: "[collapse all]",
            more_patch: "[+ more patch]",
            less_patch: "[- less patch]",
            footer_refresh: "[refresh]",
            footer_open: "[open]",
            footer_settings: "[settings]",
            footer_help: "[help]",
            footer_quit: "[quit]",
        }
    }

    fn emoji() -> Self {
        Self {
            state_open: "✅",
            state_merged: "🟣",
            state_closed: "❌",
            state_unknown: "○",
            checks_pass: "✅",
            checks_fail: "❌",
            checks_pending: "⏳",
            checks_unknown: "○",
            check_success: "✅",
            check_failure: "❌",
            check_pending: "⏳",
            check_skipped: "↷",
            check_neutral: "○",
            check_unknown: "?",
            author: "👤",
            comments: "💬",
            reactions: "👍",
            assignees: "🎯",
            threads: "🧵",
            files: "📄",
            warning: "⚠",
            refresh: "🔁",
            changed: "✦",
            error: "❌",
            info: "ⓘ",
            body: "📝",
            activity_comment: "💬",
            activity_review: "✅",
            activity_review_comment: "🧵",
            activity_commit_comment: "💭",
            activity_timeline: "•",
            more: "[➕ more]",
            less: "[➖ less]",
            expand_all: "[➕ all]",
            collapse_all: "[➖ all]",
            more_patch: "[➕ more patch]",
            less_patch: "[➖ less patch]",
            footer_refresh: "[🔄 refresh]",
            footer_open: "[🌐 open]",
            footer_settings: "[⚙ settings]",
            footer_help: "[❔ help]",
            footer_quit: "[⏻ quit]",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_symbol_modes() {
        assert_eq!("ascii".parse::<SymbolMode>().unwrap(), SymbolMode::Ascii);
        assert_eq!("plain".parse::<SymbolMode>().unwrap(), SymbolMode::Ascii);
        assert_eq!("emoji".parse::<SymbolMode>().unwrap(), SymbolMode::Emoji);
        assert!("unknown".parse::<SymbolMode>().is_err());
    }
}
