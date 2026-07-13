//! Friendly timestamp parsing and application.
//!
//! Times are interpreted in the *local* timezone (attach an offset or use
//! `@epoch` for absolute time). Accepted forms:
//!
//! - `now`, `today`, `yesterday`, `tomorrow`
//! - relative offsets: `-2h`, `+30m`, `-1d`, `2 hours ago`, `in 3 days`
//! - epoch seconds: `@1700000000`
//! - RFC 3339: `2024-01-01T09:30:00+02:00`
//! - ISO-ish dates and times: `2024-01-01 09:30[:ss]`, `2024-01-01`, `14:30`

use std::path::Path;
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use filetime::FileTime;

use crate::cli::Cli;

/// The access/modification times a run should apply, if any were requested.
#[derive(Debug, Clone, Copy)]
pub struct RequestedTimes {
    pub atime: FileTime,
    pub mtime: FileTime,
}

/// Resolve `-t/--at` or `-r/--reference` once per invocation.
pub fn resolve_requested_times(cli: &Cli) -> Result<Option<RequestedTimes>> {
    if let Some(reference) = &cli.reference {
        let md = std::fs::metadata(reference)
            .with_context(|| format!("cannot read reference file '{reference}'"))?;
        return Ok(Some(RequestedTimes {
            atime: FileTime::from_last_access_time(&md),
            mtime: FileTime::from_last_modification_time(&md),
        }));
    }

    if let Some(spec) = &cli.at {
        let t = FileTime::from_system_time(parse_when(spec)?);
        return Ok(Some(RequestedTimes { atime: t, mtime: t }));
    }

    Ok(None)
}

/// Apply times to a path, honouring `-a`/`-m` selectors. When no explicit
/// time was requested, "now" is used (classic touch).
pub fn apply_times(
    path: &Path,
    requested: Option<RequestedTimes>,
    atime_only: bool,
    mtime_only: bool,
) -> Result<()> {
    let now = FileTime::now();
    let times = requested.unwrap_or(RequestedTimes {
        atime: now,
        mtime: now,
    });

    // Neither selector means both, like touch.
    let (set_atime, set_mtime) = if atime_only || mtime_only {
        (atime_only, mtime_only)
    } else {
        (true, true)
    };

    let result = match (set_atime, set_mtime) {
        (true, true) => filetime::set_file_times(path, times.atime, times.mtime),
        (true, false) => filetime::set_file_atime(path, times.atime),
        (false, true) => filetime::set_file_mtime(path, times.mtime),
        (false, false) => unreachable!(),
    };
    result.with_context(|| format!("failed to set times on '{}'", path.display()))
}

/// Parse a human-friendly time expression into a `SystemTime`.
pub fn parse_when(input: &str) -> Result<SystemTime> {
    let s = input.trim();
    let now = Local::now();

    match s.to_ascii_lowercase().as_str() {
        "now" | "today" => return Ok(now.into()),
        "yesterday" => return Ok((now - Duration::days(1)).into()),
        "tomorrow" => return Ok((now + Duration::days(1)).into()),
        _ => {}
    }

    if let Some(epoch) = s.strip_prefix('@') {
        let secs: i64 = epoch
            .parse()
            .map_err(|_| anyhow!("'@{epoch}' is not a valid epoch timestamp"))?;
        return Ok(SystemTime::from(
            DateTime::from_timestamp(secs, 0)
                .ok_or_else(|| anyhow!("epoch timestamp '@{epoch}' is out of range"))?,
        ));
    }

    if let Some(t) = parse_relative(s, &now) {
        return Ok(t);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.into());
    }

    const DATETIME_FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ];
    for fmt in DATETIME_FORMATS {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return to_local(ndt);
        }
    }

    for fmt in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            let ndt = date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| anyhow!("invalid date '{s}'"))?;
            return to_local(ndt);
        }
    }

    for fmt in ["%H:%M:%S", "%H:%M"] {
        if let Ok(time) = NaiveTime::parse_from_str(s, fmt) {
            return to_local(now.date_naive().and_time(time));
        }
    }

    Err(anyhow!(
        "could not understand time '{input}'\n\
         try: 'now', 'yesterday', '-2h', '2 hours ago', '@1700000000',\n\
         '2024-01-01', '2024-01-01 09:30', '14:30', or RFC 3339"
    ))
}

fn to_local(ndt: NaiveDateTime) -> Result<SystemTime> {
    Local
        .from_local_datetime(&ndt)
        .earliest()
        .map(Into::into)
        .ok_or_else(|| anyhow!("'{ndt}' does not exist in the local timezone (DST gap)"))
}

fn parse_relative(s: &str, now: &DateTime<Local>) -> Option<SystemTime> {
    let lower = s.to_ascii_lowercase();

    let (sign, body) = if let Some(rest) = lower.strip_prefix('+') {
        (1, rest.trim().to_string())
    } else if let Some(rest) = lower.strip_prefix('-') {
        (-1, rest.trim().to_string())
    } else if let Some(rest) = lower.strip_suffix("ago") {
        (-1, rest.trim().to_string())
    } else if let Some(rest) = lower.strip_prefix("in ") {
        (1, rest.trim().to_string())
    } else {
        return None;
    };

    let offset = parse_offset(&body)?;
    let when = if sign >= 0 {
        *now + offset
    } else {
        *now - offset
    };
    Some(when.into())
}

/// "2h", "30 min", "3 days" -> a Duration.
fn parse_offset(text: &str) -> Option<Duration> {
    let t = text.trim();
    let digits_end = t.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    let n: i64 = t[..digits_end].parse().ok()?;
    let unit = t[digits_end..].trim();

    let duration = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => Duration::seconds(n),
        "m" | "min" | "mins" | "minute" | "minutes" => Duration::minutes(n),
        "h" | "hr" | "hrs" | "hour" | "hours" => Duration::hours(n),
        "d" | "day" | "days" => Duration::days(n),
        "w" | "week" | "weeks" => Duration::weeks(n),
        _ => return None,
    };
    Some(duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    fn close_to_now_offset(t: SystemTime, offset_secs: i64) {
        let expected = if offset_secs >= 0 {
            SystemTime::now() + StdDuration::from_secs(offset_secs as u64)
        } else {
            SystemTime::now() - StdDuration::from_secs((-offset_secs) as u64)
        };
        let diff = match t.duration_since(expected) {
            Ok(d) => d,
            Err(e) => e.duration(),
        };
        assert!(diff < StdDuration::from_secs(5), "off by {diff:?}");
    }

    #[test]
    fn named_moments() {
        close_to_now_offset(parse_when("now").unwrap(), 0);
        close_to_now_offset(parse_when("yesterday").unwrap(), -86_400);
        close_to_now_offset(parse_when("Tomorrow").unwrap(), 86_400);
    }

    #[test]
    fn relative_offsets() {
        close_to_now_offset(parse_when("-2h").unwrap(), -7_200);
        close_to_now_offset(parse_when("+30m").unwrap(), 1_800);
        close_to_now_offset(parse_when("2 hours ago").unwrap(), -7_200);
        close_to_now_offset(parse_when("in 3 days").unwrap(), 3 * 86_400);
        close_to_now_offset(parse_when("-1w").unwrap(), -7 * 86_400);
    }

    #[test]
    fn epoch_seconds() {
        let t = parse_when("@1700000000").unwrap();
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(secs, 1_700_000_000);
    }

    #[test]
    fn dates_are_interpreted_in_local_time() {
        let t = parse_when("2024-06-15 12:00:00").unwrap();
        let expected: SystemTime = Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2024, 6, 15)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .into();
        assert_eq!(t, expected);
    }

    #[test]
    fn date_only_means_local_midnight() {
        let t = parse_when("2024-06-15").unwrap();
        let expected: SystemTime = Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2024, 6, 15)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .into();
        assert_eq!(t, expected);
    }

    #[test]
    fn rfc3339_keeps_its_offset() {
        let t = parse_when("2024-01-01T00:00:00Z").unwrap();
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(secs, 1_704_067_200);
    }

    #[test]
    fn nonsense_is_rejected_with_guidance() {
        let err = parse_when("half past a monkey").unwrap_err().to_string();
        assert!(err.contains("could not understand"));
        assert!(err.contains("yesterday"));
    }

    #[test]
    fn ambiguous_slash_dates_are_not_guessed() {
        // v1 guessed between %d/%m/%Y and %m/%d/%Y; we refuse instead.
        assert!(parse_when("05/06/2024 10:00:00").is_err());
    }
}
