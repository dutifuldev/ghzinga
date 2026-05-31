use unicode_width::UnicodeWidthStr;

pub fn wrap_plain_text(input: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for raw_line in input.lines() {
        let cleaned = simplify_markdown_line(raw_line);
        wrap_line(&cleaned, width, &mut lines);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn wrap_display_text(input: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for raw_line in input.lines() {
        wrap_line(raw_line, width, &mut lines);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn visible_prefix(lines: &[String], max_lines: usize, expanded: bool) -> (Vec<String>, bool) {
    if expanded || lines.len() <= max_lines {
        return (lines.to_vec(), false);
    }
    (lines.iter().take(max_lines).cloned().collect(), true)
}

fn simplify_markdown_line(line: &str) -> String {
    let trimmed = line.trim();
    let without_heading = trimmed.trim_start_matches('#').trim_start();
    let without_quote = without_heading.trim_start_matches('>').trim_start();
    without_quote
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .replace("- [x]", "[x]")
        .replace("- [ ]", "[ ]")
}

fn wrap_line(input: &str, width: usize, out: &mut Vec<String>) {
    if input.is_empty() {
        out.push(String::new());
        return;
    }

    let mut current = String::new();
    for word in input.split_whitespace() {
        if UnicodeWidthStr::width(word) > width {
            if !current.is_empty() {
                out.push(current);
                current = String::new();
            }
            out.extend(split_long_word(word, width));
            continue;
        }
        let separator = if current.is_empty() { 0 } else { 1 };
        if UnicodeWidthStr::width(current.as_str()) + separator + UnicodeWidthStr::width(word)
            > width
            && !current.is_empty()
        {
            out.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        out.push(current);
    }
}

fn split_long_word(word: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for ch in word.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
        if current_width > 0 && current_width + ch_width > width {
            lines.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_markdown_markup() {
        assert_eq!(
            wrap_plain_text("## **Summary** with `code`", 80),
            vec!["Summary with code"]
        );
    }

    #[test]
    fn wraps_text_to_width() {
        assert_eq!(
            wrap_plain_text("one two three four", 8),
            vec!["one two", "three", "four"]
        );
    }

    #[test]
    fn wraps_long_words_to_width() {
        assert_eq!(
            wrap_plain_text("abcdef emoji🙂word", 5),
            vec!["abcde", "f", "emoji", "🙂wor", "d"]
        );
    }

    #[test]
    fn display_wrap_preserves_markup_characters() {
        assert_eq!(
            wrap_display_text("**bold** `code`", 8),
            vec!["**bold**", "`code`"]
        );
    }

    #[test]
    fn visible_prefix_reports_truncation() {
        let (visible, truncated) = visible_prefix(&["a".into(), "b".into(), "c".into()], 2, false);

        assert_eq!(visible, vec!["a", "b"]);
        assert!(truncated);
    }
}
