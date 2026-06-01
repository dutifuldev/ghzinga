use std::time::{SystemTime, UNIX_EPOCH};

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
const SECONDS_PER_MONTH: i64 = 28 * SECONDS_PER_DAY;
const SECONDS_PER_YEAR: i64 = 365 * SECONDS_PER_DAY;

pub fn compact_relative_time(value: &str) -> String {
    compact_relative_time_at(value, current_unix_seconds())
}

pub fn relative_time_phrase(value: &str) -> String {
    relative_time_phrase_at(value, current_unix_seconds())
}

fn compact_relative_time_at(value: &str, now: i64) -> String {
    let trimmed = value.trim();
    if let Some(timestamp) = parse_rfc3339_timestamp(trimmed) {
        let diff = now.saturating_sub(timestamp);
        let label = compact_duration_label(diff.unsigned_abs() as i64);
        if diff < 0 && label != "now" {
            format!("in {label}")
        } else {
            label
        }
    } else {
        normalize_compact_input(trimmed)
    }
}

fn relative_time_phrase_at(value: &str, now: i64) -> String {
    let trimmed = value.trim();
    if let Some(timestamp) = parse_rfc3339_timestamp(trimmed) {
        let diff = now.saturating_sub(timestamp);
        let label = compact_duration_label(diff.unsigned_abs() as i64);
        if label == "now" {
            "now".to_string()
        } else if diff < 0 {
            format!("in {label}")
        } else {
            format!("{label} ago")
        }
    } else if trimmed.eq_ignore_ascii_case("now")
        || trimmed.starts_with("in ")
        || trimmed.ends_with(" ago")
    {
        trimmed.to_string()
    } else if looks_compact_relative(trimmed) {
        format!("{trimmed} ago")
    } else {
        trimmed.to_string()
    }
}

fn normalize_compact_input(value: &str) -> String {
    if value.eq_ignore_ascii_case("now") || value.starts_with("in ") {
        value.to_string()
    } else if let Some(compact) = value.strip_suffix(" ago") {
        compact.to_string()
    } else {
        value.to_string()
    }
}

fn looks_compact_relative(value: &str) -> bool {
    let Some(first_non_digit) = value.find(|ch: char| !ch.is_ascii_digit()) else {
        return false;
    };
    first_non_digit > 0
        && matches!(
            &value[first_non_digit..],
            "s" | "m" | "h" | "d" | "w" | "mo" | "y"
        )
}

fn compact_duration_label(seconds: i64) -> String {
    if seconds <= 0 {
        return "now".to_string();
    }
    let (amount, unit) = if seconds >= SECONDS_PER_YEAR {
        (seconds / SECONDS_PER_YEAR, "y")
    } else if seconds >= SECONDS_PER_MONTH {
        (seconds / SECONDS_PER_MONTH, "mo")
    } else if seconds >= SECONDS_PER_WEEK {
        (seconds / SECONDS_PER_WEEK, "w")
    } else if seconds >= SECONDS_PER_DAY {
        (seconds / SECONDS_PER_DAY, "d")
    } else if seconds >= SECONDS_PER_HOUR {
        (seconds / SECONDS_PER_HOUR, "h")
    } else if seconds >= SECONDS_PER_MINUTE {
        (seconds / SECONDS_PER_MINUTE, "m")
    } else {
        (seconds, "s")
    };
    format!("{amount}{unit}")
}

fn current_unix_seconds() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().min(i64::MAX as u64) as i64,
        Err(error) => -(error.duration().as_secs().min(i64::MAX as u64) as i64),
    }
}

fn parse_rfc3339_timestamp(value: &str) -> Option<i64> {
    let date_time_separator = value.find('T').or_else(|| value.find(' '))?;
    let date = &value[..date_time_separator];
    let time_and_zone = &value[date_time_separator + 1..];
    let (year, month, day) = parse_date(date)?;
    let (time, offset_seconds) = split_time_and_offset(time_and_zone)?;
    let (hour, minute, second) = parse_time(time)?;
    let days = days_from_civil(year, month, day);
    Some(
        days.saturating_mul(SECONDS_PER_DAY)
            .saturating_add(i64::from(hour) * SECONDS_PER_HOUR)
            .saturating_add(i64::from(minute) * SECONDS_PER_MINUTE)
            .saturating_add(i64::from(second))
            .saturating_sub(offset_seconds),
    )
}

fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    if value.len() != 10
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
    {
        return None;
    }
    let year = value[0..4].parse::<i32>().ok()?;
    let month = value[5..7].parse::<u32>().ok()?;
    let day = value[8..10].parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn split_time_and_offset(value: &str) -> Option<(&str, i64)> {
    if let Some(time) = value.strip_suffix('Z') {
        return Some((time, 0));
    }
    let offset_start = value
        .char_indices()
        .skip(1)
        .find_map(|(index, ch)| matches!(ch, '+' | '-').then_some(index))?;
    let time = &value[..offset_start];
    let offset = &value[offset_start..];
    Some((time, parse_offset(offset)?))
}

fn parse_time(value: &str) -> Option<(u32, u32, u32)> {
    let time = value
        .split_once('.')
        .map_or(value, |(time, _fraction)| time);
    if time.len() != 8
        || time.as_bytes().get(2) != Some(&b':')
        || time.as_bytes().get(5) != Some(&b':')
    {
        return None;
    }
    let hour = time[0..2].parse::<u32>().ok()?;
    let minute = time[3..5].parse::<u32>().ok()?;
    let second = time[6..8].parse::<u32>().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some((hour, minute, second))
}

fn parse_offset(value: &str) -> Option<i64> {
    if value.len() != 6 || value.as_bytes().get(3) != Some(&b':') {
        return None;
    }
    let sign = match value.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let hours = value[1..3].parse::<i64>().ok()?;
    let minutes = value[4..6].parse::<i64>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * SECONDS_PER_HOUR + minutes * SECONDS_PER_MINUTE))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_now() -> i64 {
        parse_rfc3339_timestamp("2025-12-01T12:13:00Z").expect("fixed now")
    }

    #[test]
    fn compact_relative_time_uses_largest_floor_unit() {
        let now = fixed_now();
        assert_eq!(compact_relative_time_at("2025-12-01T12:12:30Z", now), "30s");
        assert_eq!(compact_relative_time_at("2025-12-01T11:13:00Z", now), "1h");
        assert_eq!(compact_relative_time_at("2025-11-29T12:13:00Z", now), "2d");
        assert_eq!(compact_relative_time_at("2025-11-10T12:13:00Z", now), "3w");
        assert_eq!(compact_relative_time_at("2025-10-06T12:13:00Z", now), "2mo");
    }

    #[test]
    fn relative_time_phrase_adds_suffix_only_for_past_times() {
        let now = fixed_now();
        assert_eq!(
            relative_time_phrase_at("2025-12-01T12:12:30Z", now),
            "30s ago"
        );
        assert_eq!(relative_time_phrase_at("2025-12-01T12:13:00Z", now), "now");
        assert_eq!(
            relative_time_phrase_at("2025-12-01T12:14:00Z", now),
            "in 1m"
        );
    }

    #[test]
    fn relative_formatters_normalize_existing_fixture_labels() {
        let now = fixed_now();
        assert_eq!(compact_relative_time_at("1mo ago", now), "1mo");
        assert_eq!(compact_relative_time_at("1mo", now), "1mo");
        assert_eq!(relative_time_phrase_at("1mo", now), "1mo ago");
        assert_eq!(relative_time_phrase_at("1mo ago", now), "1mo ago");
        assert_eq!(relative_time_phrase_at("unknown", now), "unknown");
    }

    #[test]
    fn rfc3339_parser_accepts_fractional_seconds_and_offsets() {
        assert_eq!(
            parse_rfc3339_timestamp("2025-12-01T12:13:00Z"),
            parse_rfc3339_timestamp("2025-12-01T12:13:00.123Z")
        );
        assert_eq!(
            parse_rfc3339_timestamp("2025-12-01T14:13:00+02:00"),
            parse_rfc3339_timestamp("2025-12-01T12:13:00Z")
        );
    }
}
