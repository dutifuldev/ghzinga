use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewRects {
    pub area: Rect,
    pub header: Rect,
    pub tabs: Rect,
    pub status: Rect,
    pub content: Rect,
    pub footer: Rect,
    pub wide: bool,
}

impl ViewRects {
    pub fn compute(area: Rect) -> Self {
        let mut header_height = chrome_height(area.width, &[(56, 4), (u16::MAX, 3)]);
        let mut tabs_height = chrome_height(area.width, &[(38, 3), (78, 2), (u16::MAX, 1)]);
        let mut status_height = chrome_height(area.width, &[(52, 4), (u16::MAX, 3)]);
        let mut footer_height = chrome_height(area.width, &[(52, 3), (u16::MAX, 2)]);
        let minimum_content_height = u16::from(area.height >= 8);
        let max_chrome = area.height.saturating_sub(minimum_content_height);
        while header_height + tabs_height + status_height + footer_height > max_chrome {
            if status_height > 1 {
                status_height -= 1;
            } else if footer_height > 1 {
                footer_height -= 1;
            } else if header_height > 2 {
                header_height -= 1;
            } else if tabs_height > 1 {
                tabs_height -= 1;
            } else {
                break;
            }
        }

        let body_y = area.y.saturating_add(header_height + status_height);
        let body_height = area
            .height
            .saturating_sub(header_height + status_height + footer_height);
        let wide = area.width >= 100 && body_height >= 8;
        let status = Rect::new(
            area.x,
            area.y.saturating_add(header_height),
            area.width,
            status_height,
        );
        let tabs = Rect::new(area.x, body_y, area.width, tabs_height);
        let content = Rect::new(
            area.x,
            body_y.saturating_add(tabs_height),
            area.width,
            body_height.saturating_sub(tabs_height),
        );

        Self {
            area,
            header: Rect::new(area.x, area.y, area.width, header_height),
            tabs,
            status,
            content,
            footer: Rect::new(
                area.x,
                area.y
                    .saturating_add(area.height.saturating_sub(footer_height)),
                area.width,
                footer_height,
            ),
            wide,
        }
    }
}

fn chrome_height(width: u16, breakpoints: &[(u16, u16)]) -> u16 {
    breakpoints
        .iter()
        .find_map(|(max_width, height)| (width <= *max_width).then_some(*height))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_layout_places_tabs_after_status_before_content() {
        let rects = ViewRects::compute(Rect::new(0, 0, 80, 24));

        assert!(!rects.wide);
        assert_eq!(rects.status.width, 80);
        assert_eq!(rects.status.y, rects.header.y + rects.header.height);
        assert_eq!(rects.tabs.y, rects.status.y + rects.status.height);
        assert_eq!(rects.content.y, rects.tabs.y + rects.tabs.height);
    }

    #[test]
    fn wide_layout_keeps_tabs_as_last_top_chrome_before_content() {
        let rects = ViewRects::compute(Rect::new(0, 0, 120, 36));

        assert!(rects.wide);
        assert_eq!(rects.status.x, 0);
        assert_eq!(rects.status.width, 120);
        assert_eq!(rects.tabs.y, rects.status.y + rects.status.height);
        assert_eq!(rects.content.x, 0);
        assert_eq!(rects.content.y, rects.tabs.y + rects.tabs.height);
    }

    #[test]
    fn narrow_layout_reserves_more_rows_for_wrapping_chrome() {
        let rects = ViewRects::compute(Rect::new(0, 0, 36, 24));

        assert_eq!(rects.header.height, 4);
        assert_eq!(rects.tabs.height, 3);
        assert_eq!(rects.status.height, 4);
        assert_eq!(rects.footer.height, 3);
        assert!(rects.content.height > 0);
    }

    #[test]
    fn cramped_layout_preserves_content_when_possible() {
        let rects = ViewRects::compute(Rect::new(0, 0, 40, 8));

        assert!(rects.content.height >= 1);
    }
}
