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
    pub footer_copy: &'static str,
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
            footer_copy: "[copy]",
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
            footer_copy: "[📋 copy]",
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

    fn symbol_values(symbols: Symbols) -> [&'static str; 43] {
        [
            symbols.state_open,
            symbols.state_merged,
            symbols.state_closed,
            symbols.state_unknown,
            symbols.checks_pass,
            symbols.checks_fail,
            symbols.checks_pending,
            symbols.checks_unknown,
            symbols.check_success,
            symbols.check_failure,
            symbols.check_pending,
            symbols.check_skipped,
            symbols.check_neutral,
            symbols.check_unknown,
            symbols.author,
            symbols.comments,
            symbols.reactions,
            symbols.assignees,
            symbols.threads,
            symbols.files,
            symbols.warning,
            symbols.refresh,
            symbols.changed,
            symbols.error,
            symbols.info,
            symbols.body,
            symbols.activity_comment,
            symbols.activity_review,
            symbols.activity_review_comment,
            symbols.activity_commit_comment,
            symbols.activity_timeline,
            symbols.more,
            symbols.less,
            symbols.expand_all,
            symbols.collapse_all,
            symbols.more_patch,
            symbols.less_patch,
            symbols.footer_refresh,
            symbols.footer_copy,
            symbols.footer_open,
            symbols.footer_settings,
            symbols.footer_help,
            symbols.footer_quit,
        ]
    }

    #[test]
    fn parses_symbol_modes() {
        assert_eq!("ascii".parse::<SymbolMode>().unwrap(), SymbolMode::Ascii);
        assert_eq!("plain".parse::<SymbolMode>().unwrap(), SymbolMode::Ascii);
        assert_eq!("emoji".parse::<SymbolMode>().unwrap(), SymbolMode::Emoji);
        assert!("unknown".parse::<SymbolMode>().is_err());
    }

    #[test]
    fn default_symbol_mode_is_ascii() {
        assert_eq!(SymbolMode::default(), SymbolMode::Ascii);
    }

    #[test]
    fn ascii_symbols_are_plain_terminal_text() {
        for value in symbol_values(Symbols::ascii()) {
            assert!(!value.is_empty(), "symbol labels should not be empty");
            assert!(value.is_ascii(), "ASCII symbol label {value:?}");
        }
    }

    #[test]
    fn emoji_controls_keep_text_labels() {
        let symbols = Symbols::emoji();
        for value in [
            symbols.more,
            symbols.less,
            symbols.expand_all,
            symbols.collapse_all,
            symbols.more_patch,
            symbols.less_patch,
            symbols.footer_refresh,
            symbols.footer_copy,
            symbols.footer_open,
            symbols.footer_settings,
            symbols.footer_help,
            symbols.footer_quit,
        ] {
            assert!(
                value.chars().any(|ch| ch.is_ascii_alphabetic()),
                "emoji control {value:?} should retain a text label"
            );
            assert!(
                value.starts_with('[') && value.ends_with(']'),
                "emoji control {value:?} should remain visibly button-like"
            );
        }
    }
}
