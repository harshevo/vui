//! Numbers and dates as people read them: `1.2k`, `3.4%`, `$5.0M`, `12 Mar`.
//!
//! No locale and no time zone: dates are UTC, numbers use a point. The calendar maths is the standard
//! days-from-civil algorithm, so there is no date library behind it.

/// Days since 1970-01-01 for a calendar date (proleptic Gregorian).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The calendar date of a day count since 1970-01-01: `(year, month 1–12, day 1–31)`.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Seconds since the epoch for a UTC date and time of day.
pub fn timestamp(y: i64, m: i64, d: i64, h: i64, min: i64, s: i64) -> i64 {
    days_from_civil(y, m, d) * 86_400 + h * 3600 + min * 60 + s
}

pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// How a value is written.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Format {
    /// Whatever reads best for the magnitude.
    Auto,
    /// Plain number with this many decimals.
    Fixed(usize),
    /// 1.2k, 3.4M, 5.6B: three significant digits.
    Si,
    /// A fraction as a percentage: 0.034 → 3.4%.
    Percent(usize),
    /// A value already in percent: 3.4 → 3.4%.
    PercentPoints(usize),
    /// A symbol and SI shortening: $1.2M.
    Money(&'static str),
    /// A UTC timestamp in seconds as `12 Mar 2024`.
    Date,
}

impl Format {
    pub fn apply(self, v: f64) -> String {
        match self {
            Format::Auto => auto(v),
            Format::Fixed(d) => trim(&format!("{v:.d$}")),
            Format::Si => si(v),
            Format::Percent(d) => format!("{}%", trim(&format!("{:.d$}", v * 100.0))),
            Format::PercentPoints(d) => format!("{}%", trim(&format!("{v:.d$}"))),
            Format::Money(sym) => {
                let s = si(v.abs());
                if v < 0.0 {
                    format!("-{sym}{s}")
                } else {
                    format!("{sym}{s}")
                }
            }
            Format::Date => date(v as i64),
        }
    }
}

fn trim(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

fn auto(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 {
        "0".into()
    } else if a >= 1e15 || a < 1e-4 {
        format!("{v:.2e}")
    } else if a >= 1000.0 {
        si(v)
    } else if a >= 100.0 {
        trim(&format!("{v:.1}"))
    } else if a >= 1.0 {
        trim(&format!("{v:.2}"))
    } else {
        trim(&format!("{v:.4}"))
    }
}

/// Three significant digits with a magnitude suffix.
pub fn si(v: f64) -> String {
    let a = v.abs();
    let (div, suffix) = if a >= 1e12 {
        (1e12, "T")
    } else if a >= 1e9 {
        (1e9, "B")
    } else if a >= 1e6 {
        (1e6, "M")
    } else if a >= 1e3 {
        (1e3, "k")
    } else {
        (1.0, "")
    };
    let x = v / div;
    let decimals = if x.abs() >= 100.0 {
        0
    } else if x.abs() >= 10.0 {
        1
    } else if suffix.is_empty() && x.abs() < 1.0 && x != 0.0 {
        3
    } else {
        2
    };
    format!("{}{suffix}", trim(&format!("{x:.decimals$}")))
}

/// `12 Mar 2024` from a UTC timestamp.
pub fn date(ts: i64) -> String {
    let (y, m, d) = civil_from_days(ts.div_euclid(86_400));
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// `12 Mar 2024, 14:30`.
pub fn date_time(ts: i64) -> String {
    let secs = ts.rem_euclid(86_400);
    format!("{}, {:02}:{:02}", date(ts), secs / 3600, secs % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_round_trips_and_knows_leap_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 1),
            29
        );
        assert_eq!(
            days_from_civil(2023, 3, 1) - days_from_civil(2023, 2, 1),
            28
        );
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(timestamp(2001, 9, 9, 1, 46, 40), 1_000_000_000);
        for day in (-800_000..800_000).step_by(997) {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
    }

    #[test]
    fn numbers_read_the_way_people_say_them() {
        assert_eq!(Format::Si.apply(1234.0), "1.23k");
        assert_eq!(Format::Si.apply(12_500_000.0), "12.5M");
        assert_eq!(Format::Si.apply(3.2e9), "3.2B");
        assert_eq!(Format::Si.apply(0.5), "0.5");
        assert_eq!(Format::Money("$").apply(5_000_000.0), "$5M");
        assert_eq!(Format::Money("₹").apply(-1500.0), "-₹1.5k");
        assert_eq!(Format::Percent(1).apply(0.034), "3.4%");
        assert_eq!(Format::PercentPoints(2).apply(3.0), "3%");
        assert_eq!(Format::Fixed(2).apply(1.5), "1.5");
        assert_eq!(Format::Auto.apply(0.0), "0");
        assert_eq!(Format::Auto.apply(42.123), "42.12");
        assert_eq!(Format::Auto.apply(250.67), "250.7");
        assert_eq!(Format::Auto.apply(0.00123), "0.0012");
    }

    #[test]
    fn dates_are_utc_and_short() {
        assert_eq!(date(0), "1 Jan 1970");
        assert_eq!(Format::Date.apply(1_000_000_000.0), "9 Sep 2001");
        assert_eq!(date_time(1_000_000_000), "9 Sep 2001, 01:46");
        assert_eq!(date(-86_400), "31 Dec 1969");
    }
}
