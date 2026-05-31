use std::{fmt, str::FromStr};

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Default,
    SolarizedDark,
}

impl ThemeName {
    pub fn palette(self) -> Palette {
        match self {
            Self::Default => Palette::default_dark(),
            Self::SolarizedDark => Palette::solarized_dark(),
        }
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::SolarizedDark => f.write_str("solarized-dark"),
        }
    }
}

impl FromStr for ThemeName {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "solarized" | "solarized-dark" | "solarized_dark" => Ok(Self::SolarizedDark),
            _ => Err("expected one of default, solarized-dark".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub accent: Color,
    pub panel_bg: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface_dim: Color,
    pub overlay0: Color,
    pub overlay1: Color,
    pub text: Color,
    pub subtext0: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub blue: Color,
    pub teal: Color,
    pub peach: Color,
}

impl Palette {
    pub fn default_dark() -> Self {
        Self {
            accent: Color::Rgb(122, 162, 247),
            panel_bg: Color::Rgb(26, 27, 38),
            surface0: Color::Rgb(36, 40, 59),
            surface1: Color::Rgb(65, 72, 104),
            surface_dim: Color::Rgb(26, 27, 38),
            overlay0: Color::Rgb(86, 95, 137),
            overlay1: Color::Rgb(105, 113, 150),
            text: Color::Rgb(192, 202, 245),
            subtext0: Color::Rgb(169, 177, 214),
            green: Color::Rgb(158, 206, 106),
            yellow: Color::Rgb(224, 175, 104),
            red: Color::Rgb(247, 118, 142),
            blue: Color::Rgb(122, 162, 247),
            teal: Color::Rgb(125, 207, 255),
            peach: Color::Rgb(255, 158, 100),
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            accent: Color::Rgb(38, 139, 210),
            panel_bg: Color::Rgb(0, 43, 54),
            surface0: Color::Rgb(7, 54, 66),
            surface1: Color::Rgb(88, 110, 117),
            surface_dim: Color::Rgb(0, 43, 54),
            overlay0: Color::Rgb(88, 110, 117),
            overlay1: Color::Rgb(101, 123, 131),
            text: Color::Rgb(147, 161, 161),
            subtext0: Color::Rgb(131, 148, 150),
            green: Color::Rgb(133, 153, 0),
            yellow: Color::Rgb(181, 137, 0),
            red: Color::Rgb(220, 50, 47),
            blue: Color::Rgb(38, 139, 210),
            teal: Color::Rgb(42, 161, 152),
            peach: Color::Rgb(203, 75, 22),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_names() {
        assert_eq!("default".parse::<ThemeName>().unwrap(), ThemeName::Default);
        assert_eq!(
            "solarized-dark".parse::<ThemeName>().unwrap(),
            ThemeName::SolarizedDark
        );
        assert_eq!(
            "solarized_dark".parse::<ThemeName>().unwrap(),
            ThemeName::SolarizedDark
        );
    }

    #[test]
    fn solarized_dark_uses_herdr_color_roles() {
        let palette = ThemeName::SolarizedDark.palette();

        assert_eq!(palette.panel_bg, Color::Rgb(0, 43, 54));
        assert_eq!(palette.accent, Color::Rgb(38, 139, 210));
        assert_eq!(palette.red, Color::Rgb(220, 50, 47));
    }
}
