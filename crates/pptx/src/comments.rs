//! The supported PowerPoint modern-comment vocabulary (MS-PPTX, section 2.16).

pub const NS: &str = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";
pub const NS_PC: &str = "http://schemas.microsoft.com/office/powerpoint/2013/main/command";
pub const NS_AC: &str = "http://schemas.microsoft.com/office/drawing/2013/main/command";
pub const CT_COMMENTS: &str = "application/vnd.ms-powerpoint.comments+xml";
pub const CT_AUTHORS: &str = "application/vnd.ms-powerpoint.authors+xml";
pub const RT_COMMENTS: &str = "http://schemas.microsoft.com/office/2018/10/relationships/comments";
pub const RT_AUTHORS: &str = "http://schemas.microsoft.com/office/2018/10/relationships/authors";
pub const EXT_URI: &str = "{6950BFC3-D8DA-4A85-94F7-54DA5524770B}";

/// A complete XML dateTime, preserving its original precision and timezone.
pub fn valid_date_time(s: &str) -> bool {
    let Some((date, time)) = s.split_once('T') else { return false };
    let mut date = date.split('-');
    let (Some(y), Some(m), Some(d), None) = (date.next(), date.next(), date.next(), date.next()) else { return false };
    if y.len() < 4 || m.len() != 2 || d.len() != 2 || ![y, m, d].iter().all(|s| s.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()) else { return false };
    let days = match m {
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    if y == 0 || !(1..=days).contains(&d) {
        return false;
    }
    let time = if let Some(t) = time.strip_suffix('Z') {
        t
    } else if let Some(at) = time.find(['+', '-']) {
        let (t, zone) = time.split_at(at);
        let Some((h, m)) = zone.get(1..).and_then(|z| z.split_once(':')) else { return false };
        if h.len() != 2 || m.len() != 2 || ![h, m].iter().all(|s| s.bytes().all(|b| b.is_ascii_digit())) {
            return false;
        }
        let (Ok(h), Ok(m)) = (h.parse::<u32>(), m.parse::<u32>()) else { return false };
        if h > 14 || m > 59 || (h == 14 && m != 0) {
            return false;
        }
        t
    } else {
        time
    };
    let mut time = time.split(':');
    let (Some(h), Some(m), Some(s), None) = (time.next(), time.next(), time.next(), time.next()) else { return false };
    let (whole, fraction) = s.split_once('.').map_or((s, None), |(s, f)| (s, Some(f)));
    if h.len() != 2
        || m.len() != 2
        || whole.len() != 2
        || ![h, m, whole].iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
        || fraction.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(h), Ok(m), Ok(s)) = (h.parse::<u32>(), m.parse::<u32>(), whole.parse::<u32>()) else { return false };
    h <= 24 && m < 60 && s < 60 && (h != 24 || (m == 0 && s == 0 && fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_keep_fractional_seconds_and_optional_timezone() {
        for s in ["2024-02-29T01:02:03Z", "2026-01-01T24:00:00Z", "2025-02-03T04:05:06.000", "2026-01-01T00:00:00.123+05:30"] {
            assert!(valid_date_time(s), "{s}");
        }
        for s in ["", "yesterday", "2026-01-01T24:00:00.01Z", "2025-02-29T00:00:00Z", "2026-01-01T99:00:00Z", "2026-01-01T00:00:00.Z"] {
            assert!(!valid_date_time(s), "{s}");
        }
    }
}
