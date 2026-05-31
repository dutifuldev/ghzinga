use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpacingMode {
    #[default]
    Comfortable,
    Compact,
}

impl fmt::Display for SpacingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comfortable => f.write_str("comfortable"),
            Self::Compact => f.write_str("compact"),
        }
    }
}

impl FromStr for SpacingMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "comfortable" | "comfy" => Ok(Self::Comfortable),
            "compact" | "dense" => Ok(Self::Compact),
            _ => Err("expected one of comfortable, compact".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spacing_modes() {
        assert_eq!(
            "comfortable".parse::<SpacingMode>().unwrap(),
            SpacingMode::Comfortable
        );
        assert_eq!(
            "comfy".parse::<SpacingMode>().unwrap(),
            SpacingMode::Comfortable
        );
        assert_eq!(
            "compact".parse::<SpacingMode>().unwrap(),
            SpacingMode::Compact
        );
        assert_eq!(
            "dense".parse::<SpacingMode>().unwrap(),
            SpacingMode::Compact
        );
        assert!("wide".parse::<SpacingMode>().is_err());
    }
}
