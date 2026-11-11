//! Dates and times: a calendar date with the arithmetic a calendar needs, the month grid, the date
//! picker, and the time field.

mod calendar;
mod picker;
mod time;

pub use calendar::{Calendar, calendar};
pub use picker::{DateEvent, DatePicker, date_picker};
pub use time::time_field;

/// A day in the proleptic Gregorian calendar. No time zone: it is the day the person means.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to the month's length.
    pub day: u8,
}

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

impl Date {
    /// The date, if it exists: `Date::new(2026, 2, 29)` is `None`.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)).then_some(Date { year, month, day })
    }

    /// Today in UTC, from the system clock.
    pub fn today() -> Date {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Date::from_days((secs / 86_400) as i64)
    }

    /// Days since 1970-01-01.
    pub fn days(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let y = self.year as i64 - (self.month <= 2) as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` after 1970-01-01.
    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        Date { year: (yoe + era * 400 + (month <= 2) as i64) as i32, month, day }
    }

    /// 0 for Monday through 6 for Sunday.
    pub fn weekday(self) -> u8 {
        (self.days() + 3).rem_euclid(7) as u8
    }

    pub fn add_days(self, days: i64) -> Date {
        Date::from_days(self.days() + days)
    }

    /// The same day `months` later, or the month's last day if it is shorter: 31 January plus one
    /// month is 28 or 29 February.
    pub fn add_months(self, months: i32) -> Date {
        let total = self.year * 12 + (self.month as i32 - 1) + months;
        let (year, month) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u8);
        Date { year, month, day: self.day.min(days_in_month(year, month)) }
    }

    /// The first day of the date's month.
    pub fn first_of_month(self) -> Date {
        Date { day: 1, ..self }
    }

    /// "November".
    pub fn month_name(self) -> &'static str {
        MONTHS[(self.month as usize).clamp(1, 12) - 1]
    }

    /// "11 Nov 2026".
    pub fn short(self) -> String {
        format!("{} {} {}", self.day, &self.month_name()[..3], self.year)
    }

    /// "2026-11-11".
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Reads "2026-11-11".
    pub fn parse_iso(s: &str) -> Option<Date> {
        let mut parts = s.trim().splitn(3, '-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        Date::new(year, month, day)
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_and_weekdays() {
        let d = Date::new(2026, 11, 11).unwrap();
        assert_eq!(Date::from_days(d.days()), d);
        assert_eq!(Date::new(1970, 1, 1).unwrap().days(), 0);
        assert_eq!(d.weekday(), 2, "a Wednesday");
        assert_eq!(Date::new(2000, 2, 29).unwrap().weekday(), 1, "a Tuesday");
        for n in [-800_000, -1, 0, 59, 10_957, 20_000, 800_000] {
            assert_eq!(Date::from_days(n).days(), n);
        }
    }

    #[test]
    fn months_add_and_clamp() {
        let jan31 = Date::new(2028, 1, 31).unwrap();
        assert_eq!(jan31.add_months(1), Date::new(2028, 2, 29).unwrap());
        assert_eq!(jan31.add_months(-1), Date::new(2027, 12, 31).unwrap());
        assert_eq!(jan31.add_months(13).month, 2);
        assert!(Date::new(2026, 2, 29).is_none());
        assert_eq!(Date::parse_iso("2026-11-11").unwrap().iso(), "2026-11-11");
        assert_eq!(Date::new(2026, 11, 11).unwrap().short(), "11 Nov 2026");
    }
}
