//! Local wall-clock timestamps.
//!
//! TeslaCam timestamps are local wall-clock strings. They are never converted through a time zone or
//! the system calendar: all arithmetic works on proleptic Gregorian calendar fields, with
//! `0001-01-01 00:00:00` as ordinal second 0.

const SECONDS_PER_DAY: i64 = 86_400;
/// The largest integer an IEEE double holds exactly; offsets beyond it are rejected like the web's
/// `Number.isSafeInteger` check.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A validated local timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalTimestamp {
    /// Year, at least 1.
    pub year: u32,
    /// Month, 1–12.
    pub month: u32,
    /// Day of the month, 1–31.
    pub day: u32,
    /// Hour, 0–23.
    pub hour: u32,
    /// Minute, 0–59.
    pub minute: u32,
    /// Second, 0–59.
    pub second: u32,
    /// Millisecond, 0–999 (only metadata timestamps carry one; it never enters [`Self::ordinal_seconds`]).
    pub millisecond: u32,
}

/// Day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    /// 月曜日
    Monday,
    /// 火曜日
    Tuesday,
    /// 水曜日
    Wednesday,
    /// 木曜日
    Thursday,
    /// 金曜日
    Friday,
    /// 土曜日
    Saturday,
    /// 日曜日
    Sunday,
}

/// Gregorian leap year rule.
pub const fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Days in `month` of `year` (0 for an invalid month).
pub const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn days_before_year(year: u32) -> i64 {
    let completed = i64::from(year) - 1;
    completed * 365 + completed.div_euclid(4) - completed.div_euclid(100)
        + completed.div_euclid(400)
}

fn days_before_month(year: u32, month: u32) -> i64 {
    (1..month)
        .map(|month| i64::from(days_in_month(year, month)))
        .sum()
}

impl LocalTimestamp {
    /// Validates calendar fields.
    pub fn new(
        year: u32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        millisecond: u32,
    ) -> Option<LocalTimestamp> {
        let valid = year >= 1
            && (1..=12).contains(&month)
            && day >= 1
            && day <= days_in_month(year, month)
            && hour < 24
            && minute < 60
            && second < 60
            && millisecond < 1_000;
        valid.then_some(LocalTimestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        })
    }

    /// `YYYY-MM-DD` (zero-padded; years above 9999 keep all digits).
    pub fn date_key(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `hh:mm:ss`.
    pub fn time_label(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }

    /// `hh:mm`.
    pub fn hour_minute_label(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// Seconds since local midnight.
    pub fn seconds_of_day(&self) -> u32 {
        self.hour * 3_600 + self.minute * 60 + self.second
    }

    /// Days since `0001-01-01`.
    pub fn ordinal_days(&self) -> i64 {
        days_before_year(self.year) + days_before_month(self.year, self.month) + i64::from(self.day)
            - 1
    }

    /// Seconds since `0001-01-01 00:00:00` (milliseconds are not included).
    pub fn ordinal_seconds(&self) -> i64 {
        self.ordinal_days() * SECONDS_PER_DAY + i64::from(self.seconds_of_day())
    }

    /// Day of the week (`0001-01-01` is a Monday in the proleptic Gregorian calendar).
    pub fn weekday(&self) -> Weekday {
        const DAYS: [Weekday; 7] = [
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ];
        DAYS[self.ordinal_days().rem_euclid(7) as usize]
    }

    /// The timestamp at `ordinal_seconds` (see [`Self::ordinal_seconds`]); `None` when negative.
    pub fn from_ordinal_seconds(ordinal_seconds: i64) -> Option<LocalTimestamp> {
        if ordinal_seconds < 0 {
            return None;
        }
        let ordinal_days = ordinal_seconds.div_euclid(SECONDS_PER_DAY);
        let seconds_of_day = ordinal_seconds - ordinal_days * SECONDS_PER_DAY;
        let mut year = u32::try_from((ordinal_days as f64 / 365.2425) as i64 + 1).ok()?;
        while days_before_year(year) > ordinal_days {
            year -= 1;
        }
        while days_before_year(year + 1) <= ordinal_days {
            year += 1;
        }
        let mut remaining = ordinal_days - days_before_year(year);
        let mut month = 1;
        while remaining >= i64::from(days_in_month(year, month)) {
            remaining -= i64::from(days_in_month(year, month));
            month += 1;
        }
        let seconds_of_day = u32::try_from(seconds_of_day).ok()?;
        LocalTimestamp::new(
            year,
            month,
            u32::try_from(remaining).ok()? + 1,
            seconds_of_day / 3_600,
            seconds_of_day % 3_600 / 60,
            seconds_of_day % 60,
            0,
        )
    }

    /// This timestamp shifted by whole seconds (milliseconds are dropped).
    pub fn plus_seconds(&self, seconds: i64) -> Option<LocalTimestamp> {
        LocalTimestamp::from_ordinal_seconds(self.ordinal_seconds().checked_add(seconds)?)
    }

    /// `YYYY-MM-DD hh:mm:ss`.
    pub fn date_time_label(&self) -> String {
        format!("{} {}", self.date_key(), self.time_label())
    }
}

/// Reads exactly `count` ASCII digits starting at `start`.
fn digits(bytes: &[u8], start: usize, count: usize) -> Option<u32> {
    let slice = bytes.get(start..start + count)?;
    slice.iter().try_fold(0u32, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u32::from(byte - b'0'))
    })
}

/// Parses `YYYY-MM-DD?hh?mm?ss` where `?` are the given separators, returning the fields.
fn date_time_fields(bytes: &[u8], separators: [u8; 5]) -> Option<[u32; 6]> {
    let layout = [(0, 4), (5, 2), (8, 2), (11, 2), (14, 2), (17, 2)];
    let separator_positions = [4, 7, 10, 13, 16];
    for (position, separator) in separator_positions.into_iter().zip(separators) {
        if bytes.get(position) != Some(&separator) {
            return None;
        }
    }
    let mut fields = [0; 6];
    for (field, (start, count)) in fields.iter_mut().zip(layout) {
        *field = digits(bytes, start, count)?;
    }
    Some(fields)
}

/// Parses a clip file-name timestamp `YYYY-MM-DD_hh-mm-ss` (ASCII digits only).
pub fn parse_filename_local_timestamp(value: &str) -> Option<LocalTimestamp> {
    let bytes = value.as_bytes();
    if bytes.len() != 19 {
        return None;
    }
    let [year, month, day, hour, minute, second] = date_time_fields(bytes, *b"--_--")?;
    LocalTimestamp::new(year, month, day, hour, minute, second, 0)
}

/// Parses an `event.json` timestamp `YYYY-MM-DDThh:mm:ss[.f{1,3}][Z]`.
///
/// The fraction is right-padded to milliseconds (`.25` → 250). A trailing `Z` is accepted and ignored
/// (no UTC conversion); offsets such as `+09:00` are rejected.
pub fn parse_metadata_local_timestamp(value: &str) -> Option<LocalTimestamp> {
    let bytes = value.as_bytes();
    let [year, month, day, hour, minute, second] = date_time_fields(bytes, *b"--T::")?;
    let mut rest = &bytes[19..];
    let mut millisecond = 0;
    if let Some(fraction) = rest.strip_prefix(b".") {
        let length = fraction
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if !(1..=3).contains(&length) {
            return None;
        }
        let parsed = digits(fraction, 0, length)?;
        millisecond = parsed * 10u32.pow(3 - length as u32);
        rest = &fraction[length..];
    }
    if let Some(after_zone) = rest.strip_prefix(b"Z") {
        rest = after_zone;
    }
    if !rest.is_empty() {
        return None;
    }
    LocalTimestamp::new(year, month, day, hour, minute, second, millisecond)
}

/// `YYYY-MM-DD hh:mm:ss` of a file-name timestamp shifted by `offset_seconds`.
///
/// `None` when the offset is not an integer, the timestamp is invalid, or the result is before year 1
/// or outside the exactly representable range.
pub fn format_filename_timestamp_at_offset(value: &str, offset_seconds: f64) -> Option<String> {
    if !offset_seconds.is_finite() || offset_seconds.fract() != 0.0 {
        return None;
    }
    let timestamp = parse_filename_local_timestamp(value)?;
    let shifted = timestamp.ordinal_seconds() as f64 + offset_seconds;
    if !(0.0..=MAX_SAFE_INTEGER).contains(&shifted) {
        return None;
    }
    LocalTimestamp::from_ordinal_seconds(shifted as i64).map(|shifted| shifted.date_time_label())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{number_input, option_json, run_fixture};

    fn timestamp_json(timestamp: Option<LocalTimestamp>) -> Value {
        timestamp.map_or(Value::Null, |timestamp| {
            json!({
                "year": timestamp.year,
                "month": timestamp.month,
                "day": timestamp.day,
                "hour": timestamp.hour,
                "minute": timestamp.minute,
                "second": timestamp.second,
                "millisecond": timestamp.millisecond,
                "dateKey": timestamp.date_key(),
                "timeLabel": timestamp.time_label(),
                "secondsOfDay": timestamp.seconds_of_day(),
                "ordinalSeconds": timestamp.ordinal_seconds(),
            })
        })
    }

    #[test]
    fn matches_the_local_timestamp_fixture() {
        run_fixture("local-timestamp", |case| {
            let value = case.input("value").as_str().expect("value");
            Some(match case.function.as_str() {
                "parseFilenameLocalTimestamp" => {
                    timestamp_json(parse_filename_local_timestamp(value))
                }
                "parseMetadataLocalTimestamp" => {
                    timestamp_json(parse_metadata_local_timestamp(value))
                }
                "formatFilenameTimestampAtOffset" => {
                    option_json(format_filename_timestamp_at_offset(
                        value,
                        number_input(case.input("offsetSeconds")),
                    ))
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn ordinal_seconds_of_known_moments() {
        let epoch = parse_filename_local_timestamp("0001-01-01_00-00-00").expect("epoch");
        assert_eq!(epoch.ordinal_seconds(), 0);
        let late = parse_filename_local_timestamp("2026-08-24_23-59-59").expect("late");
        assert_eq!(late.ordinal_seconds(), 63_923_212_799);
    }

    #[test]
    fn ordinal_round_trip_across_leap_years() {
        for value in [
            "2000-02-29_12-00-00",
            "2028-12-31_23-59-59",
            "2100-03-01_00-00-00",
            "0001-01-01_00-00-00",
        ] {
            let timestamp = parse_filename_local_timestamp(value).expect("valid");
            assert_eq!(
                LocalTimestamp::from_ordinal_seconds(timestamp.ordinal_seconds()),
                Some(timestamp)
            );
        }
        assert_eq!(LocalTimestamp::from_ordinal_seconds(-1), None);
    }

    #[test]
    fn weekdays() {
        let thursday = parse_filename_local_timestamp("2026-08-20_20-35-00").expect("valid");
        assert_eq!(thursday.weekday(), Weekday::Thursday);
        let epoch = parse_filename_local_timestamp("0001-01-01_00-00-00").expect("valid");
        assert_eq!(epoch.weekday(), Weekday::Monday);
        let sunday = parse_filename_local_timestamp("2026-10-18_00-00-00").expect("valid");
        assert_eq!(sunday.weekday(), Weekday::Sunday);
    }

    #[test]
    fn digits_must_be_ascii() {
        assert_eq!(
            parse_filename_local_timestamp("２０２６-08-24_12-00-30"),
            None
        );
        assert_eq!(parse_metadata_local_timestamp("2026-08-24T12:00:3٠"), None);
    }

    #[test]
    fn plus_seconds_crosses_midnight() {
        let timestamp = parse_filename_local_timestamp("2026-12-31_23-59-45").expect("valid");
        assert_eq!(
            timestamp
                .plus_seconds(30)
                .map(|shifted| shifted.date_time_label()),
            Some("2027-01-01 00:00:15".to_owned())
        );
        assert_eq!(timestamp.hour_minute_label(), "23:59");
    }
}
