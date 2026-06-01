use std::{fmt, str::FromStr};

pub const DEFAULT_FIXED_CONTENT_WIDTH: u16 = 118;
pub const MIN_FIXED_CONTENT_WIDTH: u16 = 72;
pub const MAX_FIXED_CONTENT_WIDTH: u16 = 180;
pub const FIXED_CONTENT_WIDTH_STEP: u16 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentWidthMode {
    #[default]
    Fixed,
    Full,
}

impl fmt::Display for ContentWidthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed => f.write_str("fixed"),
            Self::Full => f.write_str("full"),
        }
    }
}

impl FromStr for ContentWidthMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "fixed" | "readable" | "narrow" => Ok(Self::Fixed),
            "full" | "fluid" | "wide" => Ok(Self::Full),
            _ => Err("expected one of fixed, full".to_string()),
        }
    }
}

pub fn normalize_fixed_width(width: u16) -> u16 {
    width.clamp(MIN_FIXED_CONTENT_WIDTH, MAX_FIXED_CONTENT_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_width_modes() {
        assert_eq!(
            "fixed".parse::<ContentWidthMode>().unwrap(),
            ContentWidthMode::Fixed
        );
        assert_eq!(
            "full".parse::<ContentWidthMode>().unwrap(),
            ContentWidthMode::Full
        );
        assert_eq!(
            "readable".parse::<ContentWidthMode>().unwrap(),
            ContentWidthMode::Fixed
        );
    }

    #[test]
    fn fixed_width_is_clamped_to_supported_range() {
        assert_eq!(normalize_fixed_width(1), MIN_FIXED_CONTENT_WIDTH);
        assert_eq!(normalize_fixed_width(118), 118);
        assert_eq!(normalize_fixed_width(999), MAX_FIXED_CONTENT_WIDTH);
    }
}
