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
        let header_height = if area.height >= 8 { 3 } else { 2 };
        let tabs_height = 1;
        let footer_height = 1;
        let body_y = area.y.saturating_add(header_height + tabs_height);
        let body_height = area
            .height
            .saturating_sub(header_height + tabs_height + footer_height);
        let wide = area.width >= 100 && body_height >= 8;

        let (status, content) = if wide {
            let status_width = 30.min(area.width / 3);
            let gutter_width = 1;
            (
                Rect::new(area.x, body_y, status_width, body_height),
                Rect::new(
                    area.x.saturating_add(status_width + gutter_width),
                    body_y,
                    area.width.saturating_sub(status_width + gutter_width),
                    body_height,
                ),
            )
        } else {
            (
                Rect::new(area.x, body_y, area.width, 2.min(body_height)),
                Rect::new(
                    area.x,
                    body_y.saturating_add(2.min(body_height)),
                    area.width,
                    body_height.saturating_sub(2.min(body_height)),
                ),
            )
        };

        Self {
            area,
            header: Rect::new(area.x, area.y, area.width, header_height),
            tabs: Rect::new(
                area.x,
                area.y.saturating_add(header_height),
                area.width,
                tabs_height,
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_layout_stacks_status_above_content() {
        let rects = ViewRects::compute(Rect::new(0, 0, 80, 24));

        assert!(!rects.wide);
        assert_eq!(rects.status.width, 80);
        assert_eq!(rects.content.y, rects.status.y + rects.status.height);
    }

    #[test]
    fn wide_layout_places_status_left_of_content() {
        let rects = ViewRects::compute(Rect::new(0, 0, 120, 36));

        assert!(rects.wide);
        assert_eq!(rects.status.x, 0);
        assert_eq!(rects.content.x, rects.status.width + 1);
    }
}
