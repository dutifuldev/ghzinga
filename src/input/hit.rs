use ratatui::layout::Rect;

use crate::app::{BlockId, Tab};
use crate::domain::ResourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTarget {
    Tab(Tab),
    ToggleBlock(BlockId),
    Navigate(ResourceId),
    OpenUrl(String),
    Refresh,
    OpenCurrent,
    Quit,
    Help,
    Settings,
    CloseSettings,
    SetTheme(String),
    SetSymbols(String),
}

impl HitTarget {
    pub fn is_content_action(&self) -> bool {
        matches!(
            self,
            Self::ToggleBlock(_)
                | Self::Navigate(_)
                | Self::OpenUrl(_)
                | Self::SetTheme(_)
                | Self::SetSymbols(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitArea {
    pub rect: Rect,
    pub target: HitTarget,
}

impl HitArea {
    pub fn new(rect: Rect, target: HitTarget) -> Self {
        Self { rect, target }
    }

    pub fn contains(&self, column: u16, row: u16) -> bool {
        column >= self.rect.x
            && column < self.rect.x.saturating_add(self.rect.width)
            && row >= self.rect.y
            && row < self.rect.y.saturating_add(self.rect.height)
    }
}

pub fn hit_test(areas: &[HitArea], column: u16, row: u16) -> Option<HitTarget> {
    areas
        .iter()
        .find(|area| area.contains(column, row))
        .map(|area| area.target.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_area_includes_top_left_and_excludes_bottom_right() {
        let area = HitArea::new(Rect::new(2, 3, 5, 4), HitTarget::Tab(Tab::Checks));

        assert!(area.contains(2, 3));
        assert!(area.contains(6, 6));
        assert!(!area.contains(7, 6));
        assert!(!area.contains(6, 7));
    }

    #[test]
    fn hit_test_returns_first_matching_target() {
        let areas = vec![HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(Tab::Files),
        )];

        assert_eq!(hit_test(&areas, 4, 0), Some(HitTarget::Tab(Tab::Files)));
    }
}
