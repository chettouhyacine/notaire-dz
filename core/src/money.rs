//! Money and rate arithmetic.
//!
//! Amounts are integer **centimes** of Algerian dinar; rates are exact rationals. No `f64` is
//! used anywhere: a fee computed in 2026 must reproduce the same number in 2036 from the stored
//! rule set, and floating point cannot promise that.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

/// Rounding policy. Kept configurable because practice varies `[VERIFY]`
/// (see docs/04 L8); the default is HALF_UP away from zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rounding {
    HalfUp,
    HalfEven,
    Floor,
}

impl Default for Rounding {
    fn default() -> Self {
        Rounding::HalfUp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn from_centimes(c: i64) -> Self {
        Money(c)
    }

    pub fn centimes(self) -> i64 {
        self.0
    }

    /// Parse a user-facing decimal string such as `"125000.00"` or `"125 000,5"`.
    pub fn parse(s: &str) -> Result<Self> {
        let cleaned: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
            .map(|c| if c == ',' { '.' } else { c })
            .collect();
        if cleaned.is_empty() {
            return Err(CoreError::Validation("empty amount".into()));
        }
        let neg = cleaned.starts_with('-');
        let body = cleaned.trim_start_matches(['-', '+']);
        let mut parts = body.split('.');
        let int_part = parts.next().unwrap_or("0");
        let frac_part = parts.next().unwrap_or("");
        if parts.next().is_some() {
            return Err(CoreError::Validation("malformed amount".into()));
        }
        if !int_part.chars().all(|c| c.is_ascii_digit())
            || !frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return Err(CoreError::Validation("malformed amount".into()));
        }
        if frac_part.len() > 2 {
            return Err(CoreError::Validation("too many decimals".into()));
        }
        let int_val: i64 = if int_part.is_empty() {
            0
        } else {
            int_part.parse().map_err(|_| CoreError::Validation("amount too large".into()))?
        };
        let frac_val: i64 = match frac_part.len() {
            0 => 0,
            1 => frac_part.parse::<i64>().unwrap() * 10,
            _ => frac_part.parse().map_err(|_| CoreError::Validation("bad decimals".into()))?,
        };
        let total = int_val
            .checked_mul(100)
            .and_then(|v| v.checked_add(frac_val))
            .ok_or_else(|| CoreError::Validation("amount too large".into()))?;
        Ok(Money(if neg { -total } else { total }))
    }

    pub fn checked_add(self, other: Money) -> Result<Money> {
        self.0
            .checked_add(other.0)
            .map(Money)
            .ok_or_else(|| CoreError::Validation("amount overflow".into()))
    }

    pub fn checked_sub(self, other: Money) -> Result<Money> {
        self.0
            .checked_sub(other.0)
            .map(Money)
            .ok_or_else(|| CoreError::Validation("amount overflow".into()))
    }

    /// Apply an exact rational rate (`num/den`) with the given rounding policy.
    pub fn apply_ratio(self, num: i64, den: i64, rounding: Rounding) -> Result<Money> {
        if den == 0 {
            return Err(CoreError::Validation("zero denominator".into()));
        }
        let prod = (self.0 as i128) * (num as i128);
        let den = den as i128;
        let quotient = prod / den;
        let remainder = prod % den;
        let rounded = match rounding {
            Rounding::Floor => {
                if remainder != 0 && (remainder < 0) != (den < 0) {
                    quotient - 1
                } else {
                    quotient
                }
            }
            Rounding::HalfUp => {
                let twice = (remainder.abs()) * 2;
                if twice >= den.abs() {
                    quotient + if (prod < 0) != (den < 0) { -1 } else { 1 }
                } else {
                    quotient
                }
            }
            Rounding::HalfEven => {
                let twice = remainder.abs() * 2;
                if twice > den.abs() {
                    quotient + if (prod < 0) != (den < 0) { -1 } else { 1 }
                } else if twice == den.abs() {
                    if quotient % 2 != 0 {
                        quotient + if (prod < 0) != (den < 0) { -1 } else { 1 }
                    } else {
                        quotient
                    }
                } else {
                    quotient
                }
            }
        };
        i64::try_from(rounded)
            .map(Money)
            .map_err(|_| CoreError::Validation("amount overflow".into()))
    }

    /// `1 234,56` — grouped with a non-breaking space, decimal comma (fr-DZ / ar-DZ).
    pub fn format_grouped(self) -> String {
        let neg = self.0 < 0;
        let abs = (self.0 as i128).abs();
        let dinars = abs / 100;
        let centimes = abs % 100;
        let digits = dinars.to_string();
        let mut grouped = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push('\u{a0}');
            }
            grouped.push(ch);
        }
        format!(
            "{}{},{:02}",
            if neg { "-" } else { "" },
            grouped,
            centimes
        )
    }

    pub fn format_dzd(self) -> String {
        format!("{} DA", self.format_grouped())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_input() {
        assert_eq!(Money::parse("125000.00").unwrap().centimes(), 12_500_000);
        assert_eq!(Money::parse("125 000,5").unwrap().centimes(), 12_500_050);
        assert_eq!(Money::parse("-3.25").unwrap().centimes(), -325);
        assert!(Money::parse("12.345").is_err());
        assert!(Money::parse("abc").is_err());
    }

    #[test]
    fn half_up_rounds_away_from_zero() {
        // 0.5 centimes of 1 centime at 50% -> 0.5 -> 1 (away from zero)
        assert_eq!(Money::from_centimes(1).apply_ratio(1, 2, Rounding::HalfUp).unwrap().centimes(), 1);
        assert_eq!(Money::from_centimes(-1).apply_ratio(1, 2, Rounding::HalfUp).unwrap().centimes(), -1);
        assert_eq!(Money::from_centimes(3).apply_ratio(1, 2, Rounding::HalfUp).unwrap().centimes(), 2);
    }

    #[test]
    fn half_even_ties_to_even() {
        assert_eq!(Money::from_centimes(5).apply_ratio(1, 2, Rounding::HalfEven).unwrap().centimes(), 2);
        assert_eq!(Money::from_centimes(7).apply_ratio(1, 2, Rounding::HalfEven).unwrap().centimes(), 4);
    }

    #[test]
    fn no_drift_over_many_additions() {
        let mut total = Money::ZERO;
        for _ in 0..10_000 {
            total = total.checked_add(Money::parse("0.01").unwrap()).unwrap();
        }
        assert_eq!(total.centimes(), 10_000);
    }

    #[test]
    fn formats_algerian_style() {
        assert_eq!(Money::parse("1234567.89").unwrap().format_grouped(), "1\u{a0}234\u{a0}567,89");
        assert_eq!(Money::from_centimes(-500).format_grouped(), "-5,00");
    }

    #[test]
    fn rejects_overflow_instead_of_wrapping() {
        assert!(Money::from_centimes(i64::MAX).checked_add(Money::from_centimes(1)).is_err());
    }
}
