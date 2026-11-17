//! Minimal civil-date handling.
//!
//! Rules compare dates (`valid_from` / `valid_to`) frequently and must be deterministic and
//! dependency-free, so we store a date as a day number using Howard Hinnant's civil-from-days
//! algorithm. Storage keeps ISO `YYYY-MM-DD` text for readability; comparisons use day numbers.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Date {
    /// Days since 1970-01-01.
    days: i32,
}

impl Date {
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Result<Self> {
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return Err(CoreError::Validation("date out of range".into()));
        }
        let y2 = if m <= 2 { y - 1 } else { y };
        let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
        let yoe = y2 - era * 400; // [0, 399]
        let mp = (m as i32 + 9) % 12; // Mar=0
        let doy = (153 * mp + 2) / 5 + d as i32 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let date = Date { days };
        // Reject impossible civil dates such as 2025-02-30.
        if date.to_ymd() != (y, m, d) {
            return Err(CoreError::Validation("impossible civil date".into()));
        }
        Ok(date)
    }

    /// Day number → date. Used for cheap year extraction from timestamps.
    pub fn from_days(days: i32) -> Self {
        Date { days }
    }

    pub fn to_ymd(self) -> (i32, u32, u32) {
        let z = self.days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    pub fn parse_iso(s: &str) -> Result<Self> {
        let mut it = s.split('-');
        let (y, m, d) = (
            it.next().ok_or_else(|| CoreError::Validation("bad date".into()))?,
            it.next().ok_or_else(|| CoreError::Validation("bad date".into()))?,
            it.next().ok_or_else(|| CoreError::Validation("bad date".into()))?,
        );
        if it.next().is_some() {
            return Err(CoreError::Validation("bad date".into()));
        }
        Date::from_ymd(
            y.parse().map_err(|_| CoreError::Validation("bad year".into()))?,
            m.parse().map_err(|_| CoreError::Validation("bad month".into()))?,
            d.parse().map_err(|_| CoreError::Validation("bad day".into()))?,
        )
    }

    pub fn to_iso(self) -> String {
        let (y, m, d) = self.to_ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    pub fn days(self) -> i32 {
        self.days
    }

    /// 0 = Sunday … 6 = Saturday (Algerian weekend is Friday–Saturday).
    pub fn weekday(self) -> u8 {
        (self.days + 4).rem_euclid(7) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_civil_dates() {
        for (y, m, d) in [(1970, 1, 1), (2006, 2, 20), (2008, 8, 3), (2024, 2, 29), (2100, 12, 31)] {
            let date = Date::from_ymd(y, m, d).unwrap();
            assert_eq!(date.to_ymd(), (y, m, d));
            assert_eq!(Date::parse_iso(&date.to_iso()).unwrap(), date);
        }
    }

    #[test]
    fn rejects_impossible_dates() {
        assert!(Date::from_ymd(2025, 2, 30).is_err());
        assert!(Date::from_ymd(2025, 13, 1).is_err());
    }

    #[test]
    fn orders_dates() {
        let a = Date::from_ymd(2024, 12, 31).unwrap();
        let b = Date::from_ymd(2025, 1, 1).unwrap();
        assert!(a < b);
    }

    #[test]
    fn weekday_matches_known_day() {
        // 2025-01-03 was a Friday (Algerian weekend).
        assert_eq!(Date::from_ymd(2025, 1, 3).unwrap().weekday(), 5);
    }
}
