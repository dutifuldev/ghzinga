use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarMode {
    Always,
    #[default]
    OnScroll,
    Hidden,
}

impl fmt::Display for ScrollbarMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => f.write_str("always"),
            Self::OnScroll => f.write_str("on-scroll"),
            Self::Hidden => f.write_str("hidden"),
        }
    }
}

impl FromStr for ScrollbarMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "always" | "visible" => Ok(Self::Always),
            "on-scroll" | "onscroll" | "scrolling" | "transient" | "auto" => Ok(Self::OnScroll),
            "hidden" | "hide" | "invisible" | "never" | "off" => Ok(Self::Hidden),
            _ => Err("expected one of always, on-scroll, hidden".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scrollbar_modes() {
        assert_eq!(
            "always".parse::<ScrollbarMode>().unwrap(),
            ScrollbarMode::Always
        );
        assert_eq!(
            "on_scroll".parse::<ScrollbarMode>().unwrap(),
            ScrollbarMode::OnScroll
        );
        assert_eq!(
            "hidden".parse::<ScrollbarMode>().unwrap(),
            ScrollbarMode::Hidden
        );
    }
}
