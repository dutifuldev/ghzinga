use std::{fmt, str::FromStr};

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Default,
    Catppuccin,
    CatppuccinLatte,
    Terminal,
    TokyoNight,
    TokyoNightDay,
    Dracula,
    Nord,
    Gruvbox,
    GruvboxLight,
    OneDark,
    OneLight,
    Solarized,
    SolarizedLight,
    Kanagawa,
    KanagawaLotus,
    RosePine,
    RosePineDawn,
    Vesper,
}

impl ThemeName {
    pub const ALL: &'static [Self] = &[
        Self::Default,
        Self::Catppuccin,
        Self::CatppuccinLatte,
        Self::Terminal,
        Self::TokyoNight,
        Self::TokyoNightDay,
        Self::Dracula,
        Self::Nord,
        Self::Gruvbox,
        Self::GruvboxLight,
        Self::OneDark,
        Self::OneLight,
        Self::Solarized,
        Self::SolarizedLight,
        Self::Kanagawa,
        Self::KanagawaLotus,
        Self::RosePine,
        Self::RosePineDawn,
        Self::Vesper,
    ];

    pub fn palette(self) -> Palette {
        match self {
            Self::Default | Self::TokyoNight => Palette::tokyo_night(),
            Self::Catppuccin => Palette::catppuccin(),
            Self::CatppuccinLatte => Palette::catppuccin_latte(),
            Self::Terminal => Palette::terminal(),
            Self::TokyoNightDay => Palette::tokyo_night_day(),
            Self::Dracula => Palette::dracula(),
            Self::Nord => Palette::nord(),
            Self::Gruvbox => Palette::gruvbox(),
            Self::GruvboxLight => Palette::gruvbox_light(),
            Self::OneDark => Palette::one_dark(),
            Self::OneLight => Palette::one_light(),
            Self::Solarized => Palette::solarized(),
            Self::SolarizedLight => Palette::solarized_light(),
            Self::Kanagawa => Palette::kanagawa(),
            Self::KanagawaLotus => Palette::kanagawa_lotus(),
            Self::RosePine => Palette::rose_pine(),
            Self::RosePineDawn => Palette::rose_pine_dawn(),
            Self::Vesper => Palette::vesper(),
        }
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::Catppuccin => f.write_str("catppuccin"),
            Self::CatppuccinLatte => f.write_str("catppuccin-latte"),
            Self::Terminal => f.write_str("terminal"),
            Self::TokyoNight => f.write_str("tokyo-night"),
            Self::TokyoNightDay => f.write_str("tokyo-night-day"),
            Self::Dracula => f.write_str("dracula"),
            Self::Nord => f.write_str("nord"),
            Self::Gruvbox => f.write_str("gruvbox"),
            Self::GruvboxLight => f.write_str("gruvbox-light"),
            Self::OneDark => f.write_str("one-dark"),
            Self::OneLight => f.write_str("one-light"),
            Self::Solarized => f.write_str("solarized"),
            Self::SolarizedLight => f.write_str("solarized-light"),
            Self::Kanagawa => f.write_str("kanagawa"),
            Self::KanagawaLotus => f.write_str("kanagawa-lotus"),
            Self::RosePine => f.write_str("rose-pine"),
            Self::RosePineDawn => f.write_str("rose-pine-dawn"),
            Self::Vesper => f.write_str("vesper"),
        }
    }
}

impl FromStr for ThemeName {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '_'], "-")
            .as_str()
        {
            "default" => Ok(Self::Default),
            "catppuccin" | "catppuccin-mocha" => Ok(Self::Catppuccin),
            "catppuccin-latte" | "latte" | "light" => Ok(Self::CatppuccinLatte),
            "terminal" => Ok(Self::Terminal),
            "tokyo-night" | "tokyonight" => Ok(Self::TokyoNight),
            "tokyo-night-day" | "tokyo-day" | "tokyonight-day" => Ok(Self::TokyoNightDay),
            "dracula" => Ok(Self::Dracula),
            "nord" => Ok(Self::Nord),
            "gruvbox" | "gruvbox-dark" => Ok(Self::Gruvbox),
            "gruvbox-light" => Ok(Self::GruvboxLight),
            "one-dark" | "onedark" => Ok(Self::OneDark),
            "one-light" | "onelight" => Ok(Self::OneLight),
            "solarized" | "solarized-dark" => Ok(Self::Solarized),
            "solarized-light" => Ok(Self::SolarizedLight),
            "kanagawa" => Ok(Self::Kanagawa),
            "kanagawa-lotus" | "lotus" => Ok(Self::KanagawaLotus),
            "rose-pine" | "rosepine" => Ok(Self::RosePine),
            "rose-pine-dawn" | "rosepine-dawn" | "dawn" => Ok(Self::RosePineDawn),
            "vesper" => Ok(Self::Vesper),
            _ => Err(format!(
                "expected one of {}",
                ThemeName::ALL
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
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
        Self::tokyo_night()
    }

    pub fn catppuccin() -> Self {
        Self {
            accent: Color::Rgb(137, 180, 250),
            panel_bg: Color::Rgb(24, 24, 37),
            surface0: Color::Rgb(49, 50, 68),
            surface1: Color::Rgb(69, 71, 90),
            surface_dim: Color::Rgb(30, 30, 46),
            overlay0: Color::Rgb(108, 112, 134),
            overlay1: Color::Rgb(127, 132, 156),
            text: Color::Rgb(205, 214, 244),
            subtext0: Color::Rgb(166, 173, 200),
            green: Color::Rgb(166, 227, 161),
            yellow: Color::Rgb(249, 226, 175),
            red: Color::Rgb(243, 139, 168),
            blue: Color::Rgb(137, 180, 250),
            teal: Color::Rgb(148, 226, 213),
            peach: Color::Rgb(250, 179, 135),
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            accent: Color::Rgb(30, 102, 245),
            panel_bg: Color::Rgb(239, 241, 245),
            surface0: Color::Rgb(204, 208, 218),
            surface1: Color::Rgb(188, 192, 204),
            surface_dim: Color::Rgb(230, 233, 239),
            overlay0: Color::Rgb(156, 160, 176),
            overlay1: Color::Rgb(140, 143, 161),
            text: Color::Rgb(76, 79, 105),
            subtext0: Color::Rgb(108, 111, 133),
            green: Color::Rgb(64, 160, 43),
            yellow: Color::Rgb(223, 142, 29),
            red: Color::Rgb(210, 15, 57),
            blue: Color::Rgb(30, 102, 245),
            teal: Color::Rgb(23, 146, 153),
            peach: Color::Rgb(254, 100, 11),
        }
    }

    pub fn terminal() -> Self {
        Self {
            accent: Color::Blue,
            panel_bg: Color::Reset,
            surface0: Color::Reset,
            surface1: Color::DarkGray,
            surface_dim: Color::DarkGray,
            overlay0: Color::Gray,
            overlay1: Color::White,
            text: Color::Reset,
            subtext0: Color::Gray,
            green: Color::Green,
            yellow: Color::Yellow,
            red: Color::LightRed,
            blue: Color::Blue,
            teal: Color::Cyan,
            peach: Color::Yellow,
        }
    }

    pub fn tokyo_night() -> Self {
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

    pub fn tokyo_night_day() -> Self {
        Self {
            accent: Color::Rgb(46, 125, 233),
            panel_bg: Color::Rgb(225, 226, 231),
            surface0: Color::Rgb(196, 200, 218),
            surface1: Color::Rgb(168, 174, 203),
            surface_dim: Color::Rgb(210, 211, 218),
            overlay0: Color::Rgb(137, 144, 179),
            overlay1: Color::Rgb(104, 112, 154),
            text: Color::Rgb(55, 96, 191),
            subtext0: Color::Rgb(97, 114, 176),
            green: Color::Rgb(88, 117, 57),
            yellow: Color::Rgb(140, 108, 62),
            red: Color::Rgb(245, 42, 101),
            blue: Color::Rgb(46, 125, 233),
            teal: Color::Rgb(17, 140, 116),
            peach: Color::Rgb(177, 92, 0),
        }
    }

    pub fn dracula() -> Self {
        Self {
            accent: Color::Rgb(189, 147, 249),
            panel_bg: Color::Rgb(40, 42, 54),
            surface0: Color::Rgb(68, 71, 90),
            surface1: Color::Rgb(98, 114, 164),
            surface_dim: Color::Rgb(40, 42, 54),
            overlay0: Color::Rgb(98, 114, 164),
            overlay1: Color::Rgb(130, 140, 180),
            text: Color::Rgb(248, 248, 242),
            subtext0: Color::Rgb(210, 210, 220),
            green: Color::Rgb(80, 250, 123),
            yellow: Color::Rgb(241, 250, 140),
            red: Color::Rgb(255, 85, 85),
            blue: Color::Rgb(139, 233, 253),
            teal: Color::Rgb(139, 233, 253),
            peach: Color::Rgb(255, 184, 108),
        }
    }

    pub fn nord() -> Self {
        Self {
            accent: Color::Rgb(136, 192, 208),
            panel_bg: Color::Rgb(46, 52, 64),
            surface0: Color::Rgb(59, 66, 82),
            surface1: Color::Rgb(67, 76, 94),
            surface_dim: Color::Rgb(46, 52, 64),
            overlay0: Color::Rgb(76, 86, 106),
            overlay1: Color::Rgb(100, 110, 130),
            text: Color::Rgb(236, 239, 244),
            subtext0: Color::Rgb(216, 222, 233),
            green: Color::Rgb(163, 190, 140),
            yellow: Color::Rgb(235, 203, 139),
            red: Color::Rgb(191, 97, 106),
            blue: Color::Rgb(129, 161, 193),
            teal: Color::Rgb(143, 188, 187),
            peach: Color::Rgb(208, 135, 112),
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            accent: Color::Rgb(215, 153, 33),
            panel_bg: Color::Rgb(40, 40, 40),
            surface0: Color::Rgb(60, 56, 54),
            surface1: Color::Rgb(80, 73, 69),
            surface_dim: Color::Rgb(40, 40, 40),
            overlay0: Color::Rgb(146, 131, 116),
            overlay1: Color::Rgb(168, 153, 132),
            text: Color::Rgb(235, 219, 178),
            subtext0: Color::Rgb(213, 196, 161),
            green: Color::Rgb(184, 187, 38),
            yellow: Color::Rgb(250, 189, 47),
            red: Color::Rgb(251, 73, 52),
            blue: Color::Rgb(131, 165, 152),
            teal: Color::Rgb(142, 192, 124),
            peach: Color::Rgb(254, 128, 25),
        }
    }

    pub fn gruvbox_light() -> Self {
        Self {
            accent: Color::Rgb(7, 102, 120),
            panel_bg: Color::Rgb(251, 241, 199),
            surface0: Color::Rgb(235, 219, 178),
            surface1: Color::Rgb(213, 196, 161),
            surface_dim: Color::Rgb(242, 229, 188),
            overlay0: Color::Rgb(146, 131, 116),
            overlay1: Color::Rgb(124, 111, 100),
            text: Color::Rgb(60, 56, 54),
            subtext0: Color::Rgb(80, 73, 69),
            green: Color::Rgb(121, 116, 14),
            yellow: Color::Rgb(181, 118, 20),
            red: Color::Rgb(157, 0, 6),
            blue: Color::Rgb(7, 102, 120),
            teal: Color::Rgb(66, 123, 88),
            peach: Color::Rgb(175, 58, 3),
        }
    }

    pub fn one_dark() -> Self {
        Self {
            accent: Color::Rgb(97, 175, 239),
            panel_bg: Color::Rgb(40, 44, 52),
            surface0: Color::Rgb(44, 49, 58),
            surface1: Color::Rgb(62, 68, 81),
            surface_dim: Color::Rgb(40, 44, 52),
            overlay0: Color::Rgb(92, 99, 112),
            overlay1: Color::Rgb(115, 122, 135),
            text: Color::Rgb(171, 178, 191),
            subtext0: Color::Rgb(150, 156, 168),
            green: Color::Rgb(152, 195, 121),
            yellow: Color::Rgb(229, 192, 123),
            red: Color::Rgb(224, 108, 117),
            blue: Color::Rgb(97, 175, 239),
            teal: Color::Rgb(86, 182, 194),
            peach: Color::Rgb(209, 154, 102),
        }
    }

    pub fn one_light() -> Self {
        Self {
            accent: Color::Rgb(64, 120, 242),
            panel_bg: Color::Rgb(250, 250, 250),
            surface0: Color::Rgb(240, 240, 241),
            surface1: Color::Rgb(229, 229, 230),
            surface_dim: Color::Rgb(245, 245, 246),
            overlay0: Color::Rgb(160, 161, 167),
            overlay1: Color::Rgb(104, 107, 119),
            text: Color::Rgb(56, 58, 66),
            subtext0: Color::Rgb(104, 107, 119),
            green: Color::Rgb(80, 161, 79),
            yellow: Color::Rgb(193, 132, 1),
            red: Color::Rgb(228, 86, 73),
            blue: Color::Rgb(64, 120, 242),
            teal: Color::Rgb(1, 132, 188),
            peach: Color::Rgb(152, 104, 1),
        }
    }

    pub fn solarized() -> Self {
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

    pub fn solarized_light() -> Self {
        Self {
            accent: Color::Rgb(38, 139, 210),
            panel_bg: Color::Rgb(253, 246, 227),
            surface0: Color::Rgb(238, 232, 213),
            surface1: Color::Rgb(147, 161, 161),
            surface_dim: Color::Rgb(238, 232, 213),
            overlay0: Color::Rgb(147, 161, 161),
            overlay1: Color::Rgb(88, 110, 117),
            text: Color::Rgb(101, 123, 131),
            subtext0: Color::Rgb(131, 148, 150),
            green: Color::Rgb(133, 153, 0),
            yellow: Color::Rgb(181, 137, 0),
            red: Color::Rgb(220, 50, 47),
            blue: Color::Rgb(38, 139, 210),
            teal: Color::Rgb(42, 161, 152),
            peach: Color::Rgb(203, 75, 22),
        }
    }

    pub fn kanagawa() -> Self {
        Self {
            accent: Color::Rgb(126, 156, 216),
            panel_bg: Color::Rgb(31, 31, 40),
            surface0: Color::Rgb(42, 42, 55),
            surface1: Color::Rgb(54, 54, 70),
            surface_dim: Color::Rgb(31, 31, 40),
            overlay0: Color::Rgb(114, 113, 105),
            overlay1: Color::Rgb(135, 134, 125),
            text: Color::Rgb(220, 215, 186),
            subtext0: Color::Rgb(200, 195, 170),
            green: Color::Rgb(118, 148, 106),
            yellow: Color::Rgb(192, 163, 110),
            red: Color::Rgb(195, 64, 67),
            blue: Color::Rgb(126, 156, 216),
            teal: Color::Rgb(127, 180, 202),
            peach: Color::Rgb(255, 160, 102),
        }
    }

    pub fn kanagawa_lotus() -> Self {
        Self {
            accent: Color::Rgb(77, 105, 155),
            panel_bg: Color::Rgb(242, 236, 188),
            surface0: Color::Rgb(220, 213, 172),
            surface1: Color::Rgb(201, 203, 209),
            surface_dim: Color::Rgb(213, 206, 163),
            overlay0: Color::Rgb(160, 156, 172),
            overlay1: Color::Rgb(138, 137, 128),
            text: Color::Rgb(84, 84, 100),
            subtext0: Color::Rgb(67, 67, 108),
            green: Color::Rgb(111, 137, 78),
            yellow: Color::Rgb(119, 113, 63),
            red: Color::Rgb(200, 64, 83),
            blue: Color::Rgb(77, 105, 155),
            teal: Color::Rgb(78, 140, 162),
            peach: Color::Rgb(204, 109, 0),
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            accent: Color::Rgb(196, 167, 231),
            panel_bg: Color::Rgb(25, 23, 36),
            surface0: Color::Rgb(31, 29, 46),
            surface1: Color::Rgb(38, 35, 58),
            surface_dim: Color::Rgb(25, 23, 36),
            overlay0: Color::Rgb(110, 106, 134),
            overlay1: Color::Rgb(144, 140, 170),
            text: Color::Rgb(224, 222, 244),
            subtext0: Color::Rgb(200, 197, 220),
            green: Color::Rgb(49, 116, 143),
            yellow: Color::Rgb(246, 193, 119),
            red: Color::Rgb(235, 111, 146),
            blue: Color::Rgb(49, 116, 143),
            teal: Color::Rgb(156, 207, 216),
            peach: Color::Rgb(234, 154, 151),
        }
    }

    pub fn rose_pine_dawn() -> Self {
        Self {
            accent: Color::Rgb(144, 122, 169),
            panel_bg: Color::Rgb(250, 244, 237),
            surface0: Color::Rgb(242, 233, 225),
            surface1: Color::Rgb(255, 250, 243),
            surface_dim: Color::Rgb(242, 233, 225),
            overlay0: Color::Rgb(152, 147, 165),
            overlay1: Color::Rgb(121, 117, 147),
            text: Color::Rgb(70, 66, 97),
            subtext0: Color::Rgb(121, 117, 147),
            green: Color::Rgb(40, 105, 131),
            yellow: Color::Rgb(234, 157, 52),
            red: Color::Rgb(180, 99, 122),
            blue: Color::Rgb(40, 105, 131),
            teal: Color::Rgb(86, 148, 159),
            peach: Color::Rgb(215, 130, 126),
        }
    }

    pub fn vesper() -> Self {
        Self {
            accent: Color::Rgb(255, 199, 153),
            panel_bg: Color::Rgb(26, 26, 26),
            surface0: Color::Rgb(35, 35, 35),
            surface1: Color::Rgb(40, 40, 40),
            surface_dim: Color::Rgb(16, 16, 16),
            overlay0: Color::Rgb(92, 92, 92),
            overlay1: Color::Rgb(126, 126, 126),
            text: Color::Rgb(255, 255, 255),
            subtext0: Color::Rgb(160, 160, 160),
            green: Color::Rgb(153, 255, 228),
            yellow: Color::Rgb(255, 199, 153),
            red: Color::Rgb(255, 128, 128),
            blue: Color::Rgb(176, 176, 176),
            teal: Color::Rgb(102, 221, 204),
            peach: Color::Rgb(255, 199, 153),
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
            ThemeName::Solarized
        );
        assert_eq!(
            "solarized_dark".parse::<ThemeName>().unwrap(),
            ThemeName::Solarized
        );
        assert_eq!(
            "rosepine".parse::<ThemeName>().unwrap(),
            ThemeName::RosePine
        );
        assert_eq!(
            "lotus".parse::<ThemeName>().unwrap(),
            ThemeName::KanagawaLotus
        );
    }

    #[test]
    fn herdr_builtin_theme_names_resolve() {
        for theme in ThemeName::ALL {
            assert_eq!(theme.to_string().parse::<ThemeName>().unwrap(), *theme);
        }
    }

    #[test]
    fn solarized_uses_herdr_color_roles() {
        let palette = ThemeName::Solarized.palette();

        assert_eq!(palette.panel_bg, Color::Rgb(0, 43, 54));
        assert_eq!(palette.accent, Color::Rgb(38, 139, 210));
        assert_eq!(palette.red, Color::Rgb(220, 50, 47));
    }

    #[test]
    fn every_builtin_theme_defines_complete_palette_roles() {
        for theme in ThemeName::ALL {
            let palette = theme.palette();

            if *theme != ThemeName::Terminal {
                assert_ne!(palette.text, palette.panel_bg, "{theme} text is visible");
            }
            assert_ne!(
                palette.accent, palette.panel_bg,
                "{theme} accent is visible"
            );
            assert_ne!(palette.red, palette.green, "{theme} status colors differ");
            assert_ne!(palette.yellow, palette.blue, "{theme} signal colors differ");
            assert_ne!(
                palette.teal, palette.peach,
                "{theme} secondary colors differ"
            );
            assert_ne!(
                palette.surface0, palette.overlay1,
                "{theme} depth roles differ"
            );
        }
    }
}
