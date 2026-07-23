use unicode_width::UnicodeWidthChar;

/// Multiline comment editor state. Pure text-and-cursor logic; rendering,
/// wrapping geometry, and click mapping all derive from `visual_rows` so the
/// reducer and the renderer can never disagree about where the cursor is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentComposer {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    pub scroll: usize,
    pub confirm_discard: bool,
    /// Text viewport (columns, rows) recorded by the renderer so key and
    /// click handling wrap text exactly like the last drawn frame.
    pub viewport: (u16, u16),
}

/// One display row of the wrapped composer text: a char range of a logical
/// line. Lines wrap at grapheme-width boundaries, never mid-character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualRow {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

impl CommentComposer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            ..Self::default()
        }
    }

    pub fn body(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|line| line.is_empty())
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn viewport_width(&self) -> usize {
        usize::from(self.viewport.0).max(1)
    }

    pub fn viewport_height(&self) -> usize {
        usize::from(self.viewport.1).max(1)
    }

    fn current_line(&self) -> &str {
        &self.lines[self.cursor_line]
    }

    fn touch(&mut self) {
        self.confirm_discard = false;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.touch();
        let byte = char_to_byte(self.current_line(), self.cursor_col);
        self.lines[self.cursor_line].insert(byte, ch);
        self.cursor_col += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        self.touch();
        for (index, segment) in normalize_newlines(text).split('\n').enumerate() {
            if index > 0 {
                self.newline();
            }
            for ch in segment.chars() {
                self.insert_char(ch);
            }
        }
    }

    pub fn newline(&mut self) {
        self.touch();
        let byte = char_to_byte(self.current_line(), self.cursor_col);
        let rest = self.lines[self.cursor_line].split_off(byte);
        self.lines.insert(self.cursor_line + 1, rest);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        self.touch();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            let byte = char_to_byte(self.current_line(), self.cursor_col);
            self.lines[self.cursor_line].remove(byte);
        } else if self.cursor_line > 0 {
            let removed = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.current_line().chars().count();
            self.lines[self.cursor_line].push_str(&removed);
        }
    }

    pub fn delete(&mut self) {
        self.touch();
        let line_chars = self.current_line().chars().count();
        if self.cursor_col < line_chars {
            let byte = char_to_byte(self.current_line(), self.cursor_col);
            self.lines[self.cursor_line].remove(byte);
        } else if self.cursor_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line().chars().count();
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_col < self.current_line().chars().count() {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.current_line().chars().count();
    }

    /// Move one display row up or down, holding the current display column
    /// so the cursor travels straight through wrapped lines.
    pub fn move_vertical(&mut self, width: usize, delta: isize) {
        let rows = self.visual_rows(width);
        let (row, x) = self.cursor_visual_in(&rows, width);
        let target = row.saturating_add_signed(delta).min(rows.len() - 1);
        let (line, col) = position_in_row(&self.lines, &rows[target], x);
        self.cursor_line = line;
        self.cursor_col = col;
    }

    /// Place the cursor at the char nearest to display position (row, x).
    pub fn click(&mut self, width: usize, row: usize, x: usize) {
        let rows = self.visual_rows(width);
        let target = row.min(rows.len() - 1);
        let (line, col) = position_in_row(&self.lines, &rows[target], x);
        self.cursor_line = line;
        self.cursor_col = col;
    }

    /// The text shown on one wrapped display row.
    pub fn row_text(&self, row: &VisualRow) -> &str {
        chars_slice(&self.lines[row.line], row.start, row.end)
    }

    /// Wrap every logical line into display rows of at most `width` columns.
    pub fn visual_rows(&self, width: usize) -> Vec<VisualRow> {
        let width = width.max(1);
        let mut rows = Vec::new();
        for (line_index, line) in self.lines.iter().enumerate() {
            wrap_line_into(&mut rows, line_index, line, width);
        }
        rows
    }

    /// The cursor's display position as (row, x) for the given wrap width.
    pub fn cursor_visual(&self, width: usize) -> (usize, usize) {
        let rows = self.visual_rows(width);
        self.cursor_visual_in(&rows, width)
    }

    fn cursor_visual_in(&self, rows: &[VisualRow], width: usize) -> (usize, usize) {
        let width = width.max(1);
        for (index, row) in rows.iter().enumerate() {
            if row.line != self.cursor_line || !row_holds_cursor(row, self.cursor_col, rows, index)
            {
                continue;
            }
            let x = display_width(chars_slice(
                &self.lines[row.line],
                row.start,
                self.cursor_col,
            ));
            if x < width || index + 1 == rows.len() || rows[index + 1].line != row.line {
                return (index, x.min(width.saturating_sub(1)));
            }
        }
        (rows.len().saturating_sub(1), 0)
    }

    /// Keep the cursor row inside a viewport of `height` rows: scroll may
    /// be at most the cursor row and at least one viewport above it.
    pub fn scroll_cursor_into_view(&mut self, width: usize, height: usize) {
        let (row, _) = self.cursor_visual(width);
        let min_scroll = row.saturating_add(1).saturating_sub(height.max(1));
        self.scroll = self.scroll.clamp(min_scroll, row.max(min_scroll));
    }

    pub fn scroll_by(&mut self, width: usize, height: usize, delta: isize) {
        let total = self.visual_rows(width).len();
        let max_scroll = total.saturating_sub(height.max(1));
        self.scroll = self.scroll.saturating_add_signed(delta).min(max_scroll);
    }
}

fn row_holds_cursor(row: &VisualRow, cursor_col: usize, rows: &[VisualRow], index: usize) -> bool {
    if cursor_col < row.start || cursor_col > row.end {
        return false;
    }
    // A cursor exactly at a wrap boundary belongs to the next row's start.
    let has_next_row_of_same_line = rows
        .get(index + 1)
        .is_some_and(|next| next.line == row.line);
    cursor_col < row.end || !has_next_row_of_same_line
}

fn wrap_line_into(rows: &mut Vec<VisualRow>, line_index: usize, line: &str, width: usize) {
    let mut start = 0;
    let mut column = 0;
    let mut current = 0;
    for (char_index, ch) in line.chars().enumerate() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if column + ch_width > width && column > 0 {
            rows.push(VisualRow {
                line: line_index,
                start,
                end: char_index,
            });
            start = char_index;
            column = 0;
        }
        column += ch_width;
        current = char_index + 1;
    }
    rows.push(VisualRow {
        line: line_index,
        start,
        end: current,
    });
}

fn position_in_row(lines: &[String], row: &VisualRow, x: usize) -> (usize, usize) {
    let mut column = 0;
    for (offset, ch) in chars_slice(&lines[row.line], row.start, row.end)
        .chars()
        .enumerate()
    {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if column + ch_width > x {
            return (row.line, row.start + offset);
        }
        column += ch_width;
    }
    (row.line, row.end)
}

fn chars_slice(line: &str, start: usize, end: usize) -> &str {
    let start_byte = char_to_byte(line, start);
    let end_byte = char_to_byte(line, end);
    &line[start_byte..end_byte]
}

fn char_to_byte(line: &str, char_index: usize) -> usize {
    line.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(line.len())
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0).max(1))
        .sum()
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composer_with(text: &str) -> CommentComposer {
        let mut composer = CommentComposer::new();
        composer.insert_str(text);
        composer
    }

    #[test]
    fn starts_empty_with_cursor_at_origin() {
        let composer = CommentComposer::new();
        assert!(composer.is_empty());
        assert_eq!(composer.cursor(), (0, 0));
        assert_eq!(composer.body(), "");
    }

    #[test]
    fn typing_and_newlines_build_the_body() {
        let composer = composer_with("hello\nworld");
        assert_eq!(composer.body(), "hello\nworld");
        assert_eq!(composer.cursor(), (1, 5));
    }

    #[test]
    fn paste_normalizes_crlf_line_endings() {
        let composer = composer_with("a\r\nb\rc");
        assert_eq!(composer.body(), "a\nb\nc");
    }

    #[test]
    fn insert_char_mid_line_respects_cursor() {
        let mut composer = composer_with("hllo");
        composer.move_home();
        composer.move_right();
        composer.insert_char('e');
        assert_eq!(composer.body(), "hello");
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn backspace_joins_lines_at_line_start() {
        let mut composer = composer_with("ab\ncd");
        composer.move_home();
        composer.backspace();
        assert_eq!(composer.body(), "abcd");
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn delete_joins_next_line_at_line_end() {
        let mut composer = composer_with("ab\ncd");
        composer.click(80, 0, 2);
        composer.delete();
        assert_eq!(composer.body(), "abcd");
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn backspace_removes_multibyte_chars_cleanly() {
        let mut composer = composer_with("héllo");
        composer.click(80, 0, 2);
        composer.backspace();
        assert_eq!(composer.body(), "hllo");
    }

    #[test]
    fn horizontal_movement_crosses_line_boundaries() {
        let mut composer = composer_with("ab\ncd");
        composer.click(80, 0, 2);
        composer.move_right();
        assert_eq!(composer.cursor(), (1, 0));
        composer.move_left();
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn long_lines_wrap_at_width() {
        let composer = composer_with("abcdefghij");
        let rows = composer.visual_rows(4);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].start, rows[0].end), (0, 4));
        assert_eq!((rows[1].start, rows[1].end), (4, 8));
        assert_eq!((rows[2].start, rows[2].end), (8, 10));
    }

    #[test]
    fn wide_chars_wrap_by_display_width() {
        let composer = composer_with("日本語です");
        let rows = composer.visual_rows(4);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].start, rows[0].end), (0, 2));
        assert_eq!((rows[1].start, rows[1].end), (2, 4));
    }

    #[test]
    fn cursor_visual_lands_after_wrap_boundary() {
        let composer = composer_with("abcdefghij");
        assert_eq!(composer.cursor_visual(4), (2, 2));
    }

    #[test]
    fn vertical_movement_travels_through_wrapped_rows() {
        let mut composer = composer_with("abcdefghij");
        composer.click(4, 0, 1);
        composer.move_vertical(4, 1);
        assert_eq!(composer.cursor(), (0, 5));
        composer.move_vertical(4, 1);
        assert_eq!(composer.cursor(), (0, 9));
        composer.move_vertical(4, -1);
        assert_eq!(composer.cursor(), (0, 5));
    }

    #[test]
    fn click_past_line_end_clamps_to_line_end() {
        let mut composer = composer_with("ab\ncdef");
        composer.click(80, 0, 10);
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn click_on_wide_char_selects_that_char() {
        let mut composer = composer_with("日本語");
        composer.click(80, 0, 3);
        assert_eq!(composer.cursor(), (0, 1));
    }

    #[test]
    fn scroll_follows_cursor() {
        let mut composer = composer_with("a\nb\nc\nd\ne\nf");
        composer.scroll_cursor_into_view(10, 3);
        assert_eq!(composer.scroll, 3);
        composer.click(10, 0, 0);
        composer.scroll_cursor_into_view(10, 3);
        assert_eq!(composer.scroll, 0);
    }

    #[test]
    fn scroll_by_clamps_to_content() {
        let mut composer = composer_with("a\nb\nc\nd");
        composer.scroll_by(10, 2, 10);
        assert_eq!(composer.scroll, 2);
        composer.scroll_by(10, 2, -10);
        assert_eq!(composer.scroll, 0);
    }

    #[test]
    fn backspace_at_the_origin_is_a_no_op() {
        let mut composer = composer_with("ab");
        composer.move_home();
        composer.backspace();
        assert_eq!(composer.body(), "ab");
        assert_eq!(composer.cursor(), (0, 0));
    }

    #[test]
    fn delete_at_the_end_of_the_buffer_is_a_no_op() {
        let mut composer = composer_with("ab");
        composer.delete();
        assert_eq!(composer.body(), "ab");
        assert_eq!(composer.cursor(), (0, 2));
    }

    #[test]
    fn delete_joins_exactly_the_next_line() {
        let mut composer = composer_with("aa\nbb\ncc");
        composer.click(80, 1, 2);
        composer.delete();
        assert_eq!(composer.body(), "aa\nbbcc");
        assert_eq!(composer.cursor(), (1, 2));
    }

    #[test]
    fn move_left_at_the_origin_stays_put() {
        let mut composer = composer_with("ab");
        composer.move_home();
        composer.move_left();
        assert_eq!(composer.cursor(), (0, 0));
    }

    #[test]
    fn move_left_steps_back_exactly_one_column() {
        let mut composer = composer_with("abc");
        composer.move_left();
        assert_eq!(composer.cursor(), (0, 2));
        composer.move_left();
        assert_eq!(composer.cursor(), (0, 1));
    }

    #[test]
    fn move_right_at_the_end_of_the_buffer_stays_put() {
        let mut composer = composer_with("ab\ncd");
        composer.move_right();
        assert_eq!(composer.cursor(), (1, 2));
    }

    #[test]
    fn move_home_and_end_hit_the_line_bounds() {
        let mut composer = composer_with("hello");
        composer.move_home();
        assert_eq!(composer.cursor(), (0, 0));
        composer.move_end();
        assert_eq!(composer.cursor(), (0, 5));
    }

    #[test]
    fn move_vertical_clamps_below_the_last_row() {
        let mut composer = composer_with("a\nb");
        composer.click(10, 0, 1);
        composer.move_vertical(10, 15);
        assert_eq!(composer.cursor(), (1, 1));
    }

    #[test]
    fn click_below_the_content_lands_on_the_last_row() {
        let mut composer = composer_with("abcdefgh");
        composer.click(4, 9, 1);
        assert_eq!(composer.cursor(), (0, 5));
    }

    #[test]
    fn cursor_at_an_exact_wrap_boundary_belongs_to_the_next_row() {
        let mut composer = composer_with("abcdefgh");
        composer.click(4, 1, 0);
        assert_eq!(composer.cursor(), (0, 4));
        assert_eq!(composer.cursor_visual(4), (1, 0));
    }

    #[test]
    fn cursor_on_a_full_final_row_clamps_to_the_last_cell() {
        let composer = {
            let mut composer = composer_with("abcd\nef");
            composer.click(4, 0, 4);
            composer
        };
        assert_eq!(composer.cursor(), (0, 4));
        assert_eq!(composer.cursor_visual(4), (0, 3));
    }

    #[test]
    fn cursor_visual_tracks_a_short_line_end_exactly() {
        let composer = composer_with("ab");
        assert_eq!(composer.cursor_visual(4), (0, 2));
    }

    #[test]
    fn scroll_moves_one_step_when_cursor_crosses_the_bottom_edge() {
        let mut composer = composer_with("a\nb\nc\nd");
        composer.scroll = 0;
        composer.scroll_cursor_into_view(10, 3);
        assert_eq!(composer.scroll, 1, "cursor on row 3 with height 3");
        composer.click(10, 1, 0);
        composer.scroll_cursor_into_view(10, 3);
        assert_eq!(composer.scroll, 1, "cursor already visible keeps scroll");
    }

    #[test]
    fn wide_char_exactly_filling_the_width_does_not_wrap_early() {
        let composer = composer_with("aa日");
        assert_eq!(composer.visual_rows(4).len(), 1);
        assert_eq!(composer.visual_rows(3).len(), 2);
    }

    #[test]
    fn edits_clear_the_discard_confirmation() {
        let mut composer = composer_with("draft");
        composer.confirm_discard = true;
        composer.insert_char('!');
        assert!(!composer.confirm_discard);
    }
}
