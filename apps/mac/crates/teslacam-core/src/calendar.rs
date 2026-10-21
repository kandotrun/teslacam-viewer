//! The date picker's month grid: weeks start on Sunday (Japanese wall calendars), recorded days are
//! marked, and the day on screen is selected.

use crate::copy;
use crate::day_timeline::TimelineDay;
use crate::timestamp::{LocalTimestamp, Weekday, days_in_month, parse_filename_local_timestamp};

/// A calendar month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct YearMonth {
    /// Year (1…9999).
    pub year: u32,
    /// Month (1…12).
    pub month: u32,
}

impl YearMonth {
    /// The month of a `YYYY-MM-DD` date key; `None` for an invalid date.
    pub fn of_date_key(date_key: &str) -> Option<YearMonth> {
        let date = parse_filename_local_timestamp(&format!("{date_key}_00-00-00"))?;
        Some(YearMonth {
            year: date.year,
            month: date.month,
        })
    }

    /// The month `months` later (earlier when negative); `None` outside years 1…9999.
    pub fn shifted(self, months: i32) -> Option<YearMonth> {
        let index = i64::from(self.year) * 12 + i64::from(self.month) - 1 + i64::from(months);
        let year = u32::try_from(index.div_euclid(12)).ok()?;
        let month = u32::try_from(index.rem_euclid(12)).ok()? + 1;
        (1..=9999)
            .contains(&year)
            .then_some(YearMonth { year, month })
    }

    /// The picker title, e.g. `2026年10月`.
    pub fn title(self) -> String {
        copy::month_title(self.year, self.month)
    }
}

/// One day cell of the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDay {
    /// Day of the month.
    pub day: u32,
    /// `YYYY-MM-DD`.
    pub date_key: String,
    /// Whether the day has recordings (in the rail's events).
    pub recorded: bool,
    /// Whether it is the day on screen.
    pub selected: bool,
}

/// A month laid out in weeks; cells outside the month are `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthGrid {
    /// The month.
    pub month: YearMonth,
    /// Weeks from Sunday to Saturday.
    pub weeks: Vec<[Option<CalendarDay>; 7]>,
}

fn sunday_index(weekday: Weekday) -> usize {
    match weekday {
        Weekday::Sunday => 0,
        Weekday::Monday => 1,
        Weekday::Tuesday => 2,
        Weekday::Wednesday => 3,
        Weekday::Thursday => 4,
        Weekday::Friday => 5,
        Weekday::Saturday => 6,
    }
}

/// The grid of `month` with `recorded` days marked and `selected` (a date key) highlighted.
pub fn month_grid(month: YearMonth, recorded: &[TimelineDay], selected: Option<&str>) -> MonthGrid {
    let Some(first) = LocalTimestamp::new(month.year, month.month, 1, 0, 0, 0, 0) else {
        return MonthGrid {
            month,
            weeks: Vec::new(),
        };
    };
    let mut weeks = Vec::new();
    let mut week: [Option<CalendarDay>; 7] = Default::default();
    let mut column = sunday_index(first.weekday());
    for day in 1..=days_in_month(month.year, month.month) {
        let date_key = format!("{:04}-{:02}-{:02}", month.year, month.month, day);
        week[column] = Some(CalendarDay {
            day,
            recorded: recorded.iter().any(|entry| entry.date_key == date_key),
            selected: selected == Some(date_key.as_str()),
            date_key,
        });
        column += 1;
        if column == 7 {
            weeks.push(std::mem::take(&mut week));
            column = 0;
        }
    }
    if column > 0 {
        weeks.push(week);
    }
    MonthGrid { month, weeks }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date_key: &str) -> TimelineDay {
        TimelineDay {
            date_key: date_key.to_owned(),
            event_ids: vec!["x".to_owned()],
        }
    }

    #[test]
    fn months_parse_and_shift_across_years() {
        let october = YearMonth::of_date_key("2026-10-20").expect("month");
        assert_eq!(
            october,
            YearMonth {
                year: 2026,
                month: 10
            }
        );
        assert_eq!(
            october.shifted(3),
            Some(YearMonth {
                year: 2027,
                month: 1
            })
        );
        assert_eq!(
            october.shifted(-10),
            Some(YearMonth {
                year: 2025,
                month: 12
            })
        );
        assert_eq!(YearMonth { year: 1, month: 1 }.shifted(-1), None);
        assert_eq!(YearMonth::of_date_key("2026-02-30"), None);
        assert_eq!(october.title(), "2026年10月");
    }

    #[test]
    fn weeks_start_on_sunday_and_mark_recorded_and_selected_days() {
        // 2026-10-01 is a Thursday.
        let grid = month_grid(
            YearMonth {
                year: 2026,
                month: 10,
            },
            &[day("2026-10-20"), day("2026-10-21"), day("2026-11-01")],
            Some("2026-10-21"),
        );
        assert_eq!(grid.weeks.len(), 5);
        assert!(grid.weeks[0][..4].iter().all(Option::is_none));
        let first = grid.weeks[0][4].as_ref().expect("1st");
        assert_eq!((first.day, first.date_key.as_str()), (1, "2026-10-01"));
        let tuesday = grid.weeks[3][2].as_ref().expect("20th");
        assert_eq!(tuesday.day, 20);
        assert!(tuesday.recorded && !tuesday.selected);
        let wednesday = grid.weeks[3][3].as_ref().expect("21st");
        assert!(wednesday.recorded && wednesday.selected);
        let last = grid.weeks[4][6].as_ref().expect("31st");
        assert_eq!(last.day, 31);
        assert!(!last.recorded);
    }

    #[test]
    fn a_month_starting_on_sunday_has_no_leading_blanks() {
        // 2026-02-01 is a Sunday; February 2026 fills exactly four weeks.
        let grid = month_grid(
            YearMonth {
                year: 2026,
                month: 2,
            },
            &[],
            None,
        );
        assert_eq!(grid.weeks.len(), 4);
        assert_eq!(grid.weeks[0][0].as_ref().map(|day| day.day), Some(1));
        assert_eq!(grid.weeks[3][6].as_ref().map(|day| day.day), Some(28));
    }
}
