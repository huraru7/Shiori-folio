//! frontmatterの日付の読み取り。Python版は `datetime.fromisoformat`(Python 3.12)で読み、
//! 読めなければ「日付不明」として扱う。同じ日付が同じに読めるよう、3.12が読む形をそろえる。
//! 時差の無い日時はUTCとみなす(Python版と同じ)。

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// 読めた日時(UTC)。読めなければNone。
pub fn parse(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || !s.is_ascii() {
        return None;
    }
    let (date_part, rest) = if s.len() >= 10 { s.split_at(10) } else { (s, "") };
    let date = parse_date(date_part)?;
    if rest.is_empty() {
        return Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?));
    }
    // 日付と時刻の区切りは、Python 3.12では任意の1文字(ふつうは"T"か空白)
    let rest = &rest[1..];
    let (time_str, offset) = split_offset(rest)?;
    let time = parse_time(time_str)?;
    let naive = NaiveDateTime::new(date, time);
    match offset {
        Some(off) => off.from_local_datetime(&naive).single().map(|dt| dt.with_timezone(&Utc)),
        None => Some(Utc.from_utc_datetime(&naive)),
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        return NaiveDate::from_ymd_opt(s[0..4].parse().ok()?, s[5..7].parse().ok()?, s[8..10].parse().ok()?);
    }
    if b.len() == 8 && b.iter().all(u8::is_ascii_digit) {
        return NaiveDate::from_ymd_opt(s[0..4].parse().ok()?, s[4..6].parse().ok()?, s[6..8].parse().ok()?);
    }
    None
}

/// 時刻の後ろの時差(`Z`・`+09:00`・`+0900`・`+09`)を切り分ける。
fn split_offset(s: &str) -> Option<(&str, Option<FixedOffset>)> {
    if let Some(t) = s.strip_suffix('Z') {
        return Some((t, Some(FixedOffset::east_opt(0)?)));
    }
    let Some(pos) = s.find(['+', '-']) else { return Some((s, None)) };
    let (t, off) = s.split_at(pos);
    let sign = if off.starts_with('-') { -1 } else { 1 };
    let digits: String = off[1..].chars().filter(|c| *c != ':').collect();
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (h, m, sec) = match digits.len() {
        2 => (digits[0..2].parse::<i32>().ok()?, 0, 0),
        4 => (digits[0..2].parse().ok()?, digits[2..4].parse().ok()?, 0),
        6 => (digits[0..2].parse().ok()?, digits[2..4].parse().ok()?, digits[4..6].parse().ok()?),
        _ => return None,
    };
    Some((t, Some(FixedOffset::east_opt(sign * (h * 3600 + m * 60 + sec))?)))
}

fn parse_time(s: &str) -> Option<NaiveTime> {
    let (main, frac) = match s.find(['.', ',']) {
        Some(p) => (&s[..p], Some(&s[p + 1..])),
        None => (s, None),
    };
    let nums: Vec<&str> = if main.contains(':') {
        main.split(':').collect()
    } else {
        main.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).unwrap_or("")).collect()
    };
    if nums.is_empty() || nums.len() > 3 || nums.iter().any(|n| n.len() != 2 || !n.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let h: u32 = nums[0].parse().ok()?;
    let m: u32 = nums.get(1).map_or(Some(0), |n| n.parse().ok())?;
    let sec: u32 = nums.get(2).map_or(Some(0), |n| n.parse().ok())?;
    // 小数点以下は6桁(マイクロ秒)までで切り捨てる(Python 3.12と同じ)
    let micros = match frac {
        None => 0,
        Some(f) if f.is_empty() || !f.chars().all(|c| c.is_ascii_digit()) => return None,
        Some(f) => {
            let mut d: String = f.chars().take(6).collect();
            while d.len() < 6 {
                d.push('0');
            }
            d.parse().ok()?
        }
    };
    NaiveTime::from_hms_micro_opt(h, m, sec, micros)
}

pub const DECAY_GRACE_DAYS: f64 = 30.0;
pub const DECAY_FULL_DAYS: f64 = 365.0;
pub const DECAY_FLOOR: f64 = 0.8;

/// 現在モードで点数に掛ける時間減衰の係数。直近30日は1.0、30〜365日で0.8まで下がり、そこで止まる。
/// 日付不明の記事は減衰させない。
pub fn time_decay_factor(date: &str, now: DateTime<Utc>) -> f64 {
    let Some(dt) = parse(date) else { return 1.0 };
    let days = (now - dt).num_microseconds().map_or(f64::MAX, |us| us as f64 / 1e6 / 86400.0);
    if days <= DECAY_GRACE_DAYS {
        return 1.0;
    }
    if days >= DECAY_FULL_DAYS {
        return DECAY_FLOOR;
    }
    let progress = (days - DECAY_GRACE_DAYS) / (DECAY_FULL_DAYS - DECAY_GRACE_DAYS);
    1.0 - progress * (1.0 - DECAY_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python312と同じに読む() {
        let p = |s| parse(s).map(|d| d.to_rfc3339());
        assert_eq!(p("2026-10-23T14:40:33.628450200+09:00").unwrap(), "2026-10-23T05:40:33.628450+00:00");
        assert_eq!(p("2026-10-02").unwrap(), "2026-10-02T00:00:00+00:00");
        assert_eq!(p("2026-10-23 14:40").unwrap(), "2026-10-23T14:40:00+00:00");
        assert_eq!(p("2026-10-08T13:31:00Z").unwrap(), "2026-10-08T13:31:00+00:00");
        assert!(parse("\"2026-10-10\"").is_none());
        assert!(parse("きのう").is_none());
        assert!(parse("").is_none());
    }

    #[test]
    fn 減衰() {
        let now = parse("2027-01-01T00:00:00+00:00").unwrap();
        assert_eq!(time_decay_factor("2026-12-20", now), 1.0);
        assert_eq!(time_decay_factor("2025-01-01", now), DECAY_FLOOR);
        assert_eq!(time_decay_factor("不明", now), 1.0);
        let mid = time_decay_factor("2026-07-01", now);
        assert!(mid > 0.8 && mid < 1.0);
    }
}
