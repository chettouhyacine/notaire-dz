//! Versioned rules engine.
//!
//! Every rate, threshold and duty lives in `RuleSet` / `Rule` data with `valid_from` / `valid_to`,
//! a source and a review status. Code contains no rate. A `Calculation` records the rule set ids it
//! used, so the same figure can be reproduced years later (docs/05).

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::error::{CoreError, Result};
use crate::money::{Money, Rounding};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSetKind {
    Fee,
    Tax,
    Deadline,
    Checklist,
    Succession,
}

impl RuleSetKind {
    pub fn code(self) -> &'static str {
        match self {
            RuleSetKind::Fee => "fee",
            RuleSetKind::Tax => "tax",
            RuleSetKind::Deadline => "deadline",
            RuleSetKind::Checklist => "checklist",
            RuleSetKind::Succession => "succession",
        }
    }
}

/// Review status of a rule. `Verif` renders `[VERIFY]` in the UI and blocks production mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Verif,
    Draft,
    Confirmed,
}

impl ReviewStatus {
    pub fn from_code(s: &str) -> Result<Self> {
        match s {
            "verify" => Ok(ReviewStatus::Verif),
            "draft" => Ok(ReviewStatus::Draft),
            "confirmed" => Ok(ReviewStatus::Confirmed),
            other => Err(CoreError::Rules(format!("unknown review status {other}"))),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            ReviewStatus::Verif => "verify",
            ReviewStatus::Draft => "draft",
            ReviewStatus::Confirmed => "confirmed",
        }
    }

    /// `true` when this item is still marked `[VERIFY]` (i.e. NOT confirmed).
    pub fn is_unverified(self) -> bool {
        matches!(self, ReviewStatus::Verif)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub id: String,
    pub code: String,
    pub kind: RuleSetKind,
    pub version_no: u32,
    pub valid_from: Date,
    pub valid_to: Option<Date>,
    pub source: String,
    pub status: ReviewStatus,
    pub reviewed_by: Option<String>,
    pub reviewed_on: Option<Date>,
}

impl RuleSet {
    pub fn is_effective(&self, on: Date) -> bool {
        on >= self.valid_from && self.valid_to.map(|to| on < to).unwrap_or(true)
    }

    /// A set cannot go to production while any row is unverified or unsigned-off.
    pub fn can_activate(&self) -> Result<()> {
        if self.status.is_unverified() {
            return Err(CoreError::Rules(format!(
                "rule set {} still contains [VERIFY] items",
                self.code
            )));
        }
        if self.reviewed_by.is_none() || self.reviewed_on.is_none() {
            return Err(CoreError::Rules(format!(
                "rule set {} has no notary sign-off",
                self.code
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleKind {
    Tranche,
    Flat,
    Percentage,
}

/// What a percentage rule is charged on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PercentageBase {
    /// The act value / declared price.
    ActValue,
    /// The notary fees accumulated so far (this is how VAT is charged on fees, not on duties).
    NotaryFeesSubtotal,
}

/// A rule's numeric content. Amounts are centimes; rates are exact rationals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RulePayload {
    /// A progressive bracket: only the slice of the base inside `[from, to)` is charged at
    /// `rate_num/rate_den`. Summing the brackets gives the graduated tariff, which is how a
    /// tranche tariff actually works `[VERIFY against decree 08-243]`.
    Tranche {
        from_cents: i64,
        to_cents: Option<i64>,
        rate_num: i64,
        rate_den: i64,
        min_cents: Option<i64>,
        max_cents: Option<i64>,
    },
    /// Fixed amount, independent of the base.
    Flat { amount_cents: i64 },
    /// Percentage of a defined base.
    Percentage {
        rate_num: i64,
        rate_den: i64,
        base: PercentageBase,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub rule_set_id: String,
    pub code: String,
    pub kind: RuleKind,
    /// Translation key for the printed line, never literal text.
    pub label_key: String,
    pub priority: i32,
    pub payload: RulePayload,
    pub status: ReviewStatus,
    /// `collect` (the notary collects for the State) or `verify` (the notary only checks).
    pub applies_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalculationLine {
    pub rule_id: String,
    pub label_key: String,
    pub base_cents: i64,
    pub rate_num: i64,
    pub rate_den: i64,
    pub amount_cents: i64,
    pub applies_to: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Calculation {
    pub code: String,
    pub evaluated_on: Date,
    pub rule_set_ids: Vec<String>,
    pub lines: Vec<CalculationLine>,
    pub total_cents: i64,
    pub state_levies_cents: i64,
    pub notary_fees_cents: i64,
    pub rounding: String,
    /// True when at least one contributing rule is still `[VERIFY]`.
    pub contains_unverified_rules: bool,
}

#[derive(Default)]
pub struct RulesEngine {
    pub rule_sets: Vec<RuleSet>,
    pub rules: Vec<Rule>,
}

impl RulesEngine {
    pub fn new(rule_sets: Vec<RuleSet>, rules: Vec<Rule>) -> Self {
        RulesEngine { rule_sets, rules }
    }

    /// Reject overlapping effective periods for the same set code: two live tariffs would make a
    /// calculation depend on row order.
    pub fn validate_no_overlaps(&self) -> Result<()> {
        for (i, a) in self.rule_sets.iter().enumerate() {
            for b in self.rule_sets.iter().skip(i + 1) {
                if a.code != b.code || a.kind != b.kind {
                    continue;
                }
                let a_end = a.valid_to.map(|d| d.days()).unwrap_or(i32::MAX);
                let b_end = b.valid_to.map(|d| d.days()).unwrap_or(i32::MAX);
                let overlap = a.valid_from.days() < b_end && b.valid_from.days() < a_end;
                if overlap {
                    return Err(CoreError::Rules(format!(
                        "overlapping effective dates for {} ({} and {})",
                        a.code, a.id, b.id
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn resolve(&self, code: &str, on: Date) -> Result<&RuleSet> {
        let mut found: Option<&RuleSet> = None;
        for set in self.rule_sets.iter().filter(|s| s.code == code) {
            if set.is_effective(on) {
                if found.is_some() {
                    return Err(CoreError::Rules(format!(
                        "more than one effective rule set for {code} on {}",
                        on.to_iso()
                    )));
                }
                found = Some(set);
            }
        }
        found.ok_or_else(|| {
            CoreError::Rules(format!("no rule set effective for {code} on {}", on.to_iso()))
        })
    }

    /// Is the office running on an outdated tariff? `true` when today is past every set's end.
    pub fn is_stale(&self, on: Date) -> bool {
        if self.rule_sets.is_empty() {
            return true;
        }
        !self.rule_sets.iter().any(|s| s.is_effective(on))
    }

    pub fn rules_of(&self, rule_set_id: &str) -> Vec<&Rule> {
        let mut rules: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| r.rule_set_id == rule_set_id)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Evaluate every rule of the set effective on `on`, in priority order.
    pub fn calculate(&self, code: &str, base: Money, on: Date, rounding: Rounding) -> Result<Calculation> {
        let set = self.resolve(code, on)?;
        let mut lines = Vec::new();
        let mut unverified = set.status.is_unverified();
        // Running fee subtotal, so a percentage rule can be charged on fees (e.g. VAT) rather than
        // on the act value. Rules are applied in priority order, so this is deterministic.
        let mut fees_so_far: i64 = 0;

        for rule in self.rules_of(&set.id) {
            let amount = match rule.payload {
                RulePayload::Tranche {
                    from_cents,
                    to_cents,
                    rate_num,
                    rate_den,
                    min_cents,
                    max_cents,
                } => {
                    // Charge only the slice of the base that falls inside this bracket.
                    let upper = to_cents.unwrap_or(i64::MAX);
                    let slice = if base.centimes() > from_cents {
                        base.centimes().min(upper) - from_cents
                    } else {
                        0
                    };
                    if slice <= 0 {
                        continue;
                    }
                    let mut value =
                        Money::from_centimes(slice).apply_ratio(rate_num, rate_den, rounding)?;
                    if let Some(min) = min_cents {
                        if value.centimes() < min {
                            value = Money::from_centimes(min);
                        }
                    }
                    if let Some(max) = max_cents {
                        if value.centimes() > max {
                            value = Money::from_centimes(max);
                        }
                    }
                    value
                }
                RulePayload::Flat { amount_cents } => Money::from_centimes(amount_cents),
                RulePayload::Percentage { rate_num, rate_den, base: pbase } => {
                    let charged_on = match pbase {
                        PercentageBase::ActValue => base,
                        PercentageBase::NotaryFeesSubtotal => Money::from_centimes(fees_so_far),
                    };
                    charged_on.apply_ratio(rate_num, rate_den, rounding)?
                }
            };

            if rule.status.is_unverified() {
                unverified = true;
            }
            if rule.applies_to != "collect" {
                fees_so_far = fees_so_far
                    .checked_add(amount.centimes())
                    .ok_or_else(|| CoreError::Validation("fee total overflow".into()))?;
            }
            let (rate_num, rate_den) = match rule.payload {
                RulePayload::Flat { .. } => (0, 1),
                RulePayload::Tranche { rate_num, rate_den, .. } => (rate_num, rate_den),
                RulePayload::Percentage { rate_num, rate_den, .. } => (rate_num, rate_den),
            };
            lines.push(CalculationLine {
                rule_id: rule.id.clone(),
                label_key: rule.label_key.clone(),
                base_cents: base.centimes(),
                rate_num,
                rate_den,
                amount_cents: amount.centimes(),
                applies_to: rule.applies_to.clone(),
                verified: !rule.status.is_unverified() && !set.status.is_unverified(),
            });
        }

        let mut total = Money::ZERO;
        let mut state = Money::ZERO;
        let mut fees = Money::ZERO;
        for line in &lines {
            let amount = Money::from_centimes(line.amount_cents);
            total = total.checked_add(amount)?;
            if line.applies_to == "collect" {
                state = state.checked_add(amount)?;
            } else {
                fees = fees.checked_add(amount)?;
            }
        }

        Ok(Calculation {
            code: code.to_string(),
            evaluated_on: on,
            rule_set_ids: vec![set.id.clone()],
            lines,
            total_cents: total.centimes(),
            state_levies_cents: state.centimes(),
            notary_fees_cents: fees.centimes(),
            rounding: match rounding {
                Rounding::HalfUp => "half_up",
                Rounding::HalfEven => "half_even",
                Rounding::Floor => "floor",
            }
            .to_string(),
            contains_unverified_rules: unverified,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Demo values only. They are NOT the Algerian tariff; see docs/04 L1–L2.
    const DEMO_FROM: (i32, u32, u32) = (2020, 1, 1);
    const DEMO_TO: (i32, u32, u32) = (2025, 1, 1);

    fn demo_set(id: &str, from: (i32, u32, u32), to: Option<(i32, u32, u32)>, status: ReviewStatus) -> RuleSet {
        RuleSet {
            id: id.to_string(),
            code: "SALE_COSTS_DEMO".to_string(),
            kind: RuleSetKind::Fee,
            version_no: 1,
            valid_from: Date::from_ymd(from.0, from.1, from.2).unwrap(),
            valid_to: to.map(|d| Date::from_ymd(d.0, d.1, d.2).unwrap()),
            source: "DEMO ONLY — not a legal source".to_string(),
            status,
            reviewed_by: Some("notary-demo".into()),
            reviewed_on: Some(Date::from_ymd(2020, 1, 1).unwrap()),
        }
    }

    fn demo_rules(set_id: &str) -> Vec<Rule> {
        vec![
            Rule {
                id: format!("{set_id}-t1"),
                rule_set_id: set_id.to_string(),
                code: "T1".into(),
                kind: RuleKind::Tranche,
                label_key: "calc.line.fee_tranche_1".into(),
                priority: 10,
                payload: RulePayload::Tranche {
                    from_cents: 0,
                    to_cents: Some(10_000_000), // 100 000,00 DA
                    rate_num: 5,
                    rate_den: 100, // 5%
                    min_cents: None,
                    max_cents: None,
                },
                status: ReviewStatus::Draft,
                applies_to: "notary_fee".into(),
            },
            Rule {
                id: format!("{set_id}-t2"),
                rule_set_id: set_id.to_string(),
                code: "T2".into(),
                kind: RuleKind::Tranche,
                label_key: "calc.line.fee_tranche_2".into(),
                priority: 20,
                payload: RulePayload::Tranche {
                    from_cents: 10_000_000,
                    to_cents: None,
                    rate_num: 2,
                    rate_den: 100, // 2%
                    min_cents: None,
                    max_cents: None,
                },
                status: ReviewStatus::Draft,
                applies_to: "notary_fee".into(),
            },
            Rule {
                id: format!("{set_id}-flat"),
                rule_set_id: set_id.to_string(),
                code: "FIXED".into(),
                kind: RuleKind::Flat,
                label_key: "calc.line.fixed_duty".into(),
                priority: 30,
                payload: RulePayload::Flat { amount_cents: 50_000 }, // 500,00 DA
                status: ReviewStatus::Draft,
                applies_to: "collect".into(),
            },
            Rule {
                id: format!("{set_id}-vat"),
                rule_set_id: set_id.to_string(),
                code: "VAT".into(),
                kind: RuleKind::Percentage,
                label_key: "calc.line.vat".into(),
                priority: 40,
                payload: RulePayload::Percentage {
                    rate_num: 19,
                    rate_den: 100,
                    base: PercentageBase::NotaryFeesSubtotal,
                },
                status: ReviewStatus::Draft,
                applies_to: "collect".into(),
            },
        ]
    }

    fn engine() -> RulesEngine {
        let set = demo_set("rs1", DEMO_FROM, Some(DEMO_TO), ReviewStatus::Draft);
        let rules = demo_rules("rs1");
        RulesEngine::new(vec![set], rules)
    }

    #[test]
    fn golden_tranche_calculation() {
        // Base 120 000,00 DA, graduated brackets:
        //   bracket 1 [0, 100 000,00 DA) 5%  = 5 000,00 DA
        //   bracket 2 [100 000,00 DA, …) 2%  =    400,00 DA on the 20 000,00 DA slice
        //   VAT 19% on fees 5 400,00 DA      =  1 026,00 DA
        //   fixed duty                        =    500,00 DA
        let base = Money::parse("120000.00").unwrap();
        let calc = engine()
            .calculate("SALE_COSTS_DEMO", base, Date::from_ymd(2024, 6, 1).unwrap(), Rounding::HalfUp)
            .unwrap();

        assert_eq!(calc.lines.len(), 4, "two brackets + fixed + vat");
        let t1 = calc.lines.iter().find(|l| l.label_key == "calc.line.fee_tranche_1").unwrap();
        let t2 = calc.lines.iter().find(|l| l.label_key == "calc.line.fee_tranche_2").unwrap();
        assert_eq!(t1.amount_cents, 500_000);
        assert_eq!(t2.amount_cents, 40_000);
        assert_eq!(calc.notary_fees_cents, 540_000); // 5 400,00 DA
        assert_eq!(calc.state_levies_cents, 50_000 + 102_600); // fixed + VAT on fees
        assert_eq!(calc.total_cents, 692_600);
    }

    #[test]
    fn tranche_brackets_are_half_open_and_only_charge_their_slice() {
        // Exactly 100 000,00 DA: bracket 1 is fully charged, bracket 2 has an empty slice.
        let base = Money::parse("100000.00").unwrap();
        let calc = engine()
            .calculate("SALE_COSTS_DEMO", base, Date::from_ymd(2024, 6, 1).unwrap(), Rounding::HalfUp)
            .unwrap();
        let fee_lines: Vec<_> = calc
            .lines
            .iter()
            .filter(|l| l.applies_to == "notary_fee")
            .collect();
        assert_eq!(fee_lines.len(), 1);
        assert_eq!(fee_lines[0].label_key, "calc.line.fee_tranche_1");
        assert_eq!(fee_lines[0].amount_cents, 500_000); // 5% of 100 000,00 DA
    }

    #[test]
    fn vat_is_charged_on_fees_not_on_the_act_value() {
        let base = Money::parse("200000.00").unwrap();
        let calc = engine()
            .calculate("SALE_COSTS_DEMO", base, Date::from_ymd(2024, 6, 1).unwrap(), Rounding::HalfUp)
            .unwrap();
        let vat = calc.lines.iter().find(|l| l.label_key == "calc.line.vat").unwrap();
        // fees = 5% of 100 000 DA + 2% of 100 000 DA = 5 000 + 2 000 = 7 000 DA -> VAT 1 330 DA
        assert_eq!(vat.amount_cents, 133_000);
        assert_eq!(calc.notary_fees_cents, 700_000);
    }

    #[test]
    fn effective_dates_resolve_to_the_right_set() {
        let old = demo_set("old", DEMO_FROM, Some(DEMO_TO), ReviewStatus::Draft);
        let new = demo_set("new", DEMO_TO, None, ReviewStatus::Draft);
        let engine = RulesEngine::new(vec![old, new], demo_rules("old"));

        let last_day = Date::from_ymd(2024, 12, 31).unwrap();
        let first_day = Date::from_ymd(2025, 1, 1).unwrap();
        assert_eq!(engine.resolve("SALE_COSTS_DEMO", last_day).unwrap().id, "old");
        assert_eq!(engine.resolve("SALE_COSTS_DEMO", first_day).unwrap().id, "new");
    }

    #[test]
    fn gap_in_dates_is_reported_not_guessed() {
        let old = demo_set("old", DEMO_FROM, Some(DEMO_TO), ReviewStatus::Draft);
        let engine = RulesEngine::new(vec![old], demo_rules("old"));
        let after = Date::from_ymd(2026, 1, 1).unwrap();
        assert!(engine.resolve("SALE_COSTS_DEMO", after).is_err());
        assert!(engine.is_stale(after));
    }

    #[test]
    fn overlapping_sets_are_rejected() {
        let a = demo_set("a", DEMO_FROM, None, ReviewStatus::Draft);
        let b = demo_set("b", DEMO_TO, None, ReviewStatus::Draft);
        let engine = RulesEngine::new(vec![a, b], vec![]);
        assert!(engine.validate_no_overlaps().is_err());
    }

    #[test]
    fn unverified_rules_are_flagged_on_the_calculation() {
        let set = demo_set("rs1", DEMO_FROM, Some(DEMO_TO), ReviewStatus::Verif);
        let engine = RulesEngine::new(vec![set], demo_rules("rs1"));
        let calc = engine
            .calculate(
                "SALE_COSTS_DEMO",
                Money::parse("50000.00").unwrap(),
                Date::from_ymd(2024, 6, 1).unwrap(),
                Rounding::HalfUp,
            )
            .unwrap();
        assert!(calc.contains_unverified_rules);
        assert!(calc.lines.iter().all(|l| !l.verified));
    }

    #[test]
    fn calculation_records_rule_set_for_reproduction() {
        let base = Money::parse("250000.00").unwrap();
        let on = Date::from_ymd(2024, 3, 3).unwrap();
        let first = engine().calculate("SALE_COSTS_DEMO", base, on, Rounding::HalfUp).unwrap();
        // Rebuild an engine holding only the recorded set: the figure must be identical.
        let replay = RulesEngine::new(
            vec![demo_set(&first.rule_set_ids[0], DEMO_FROM, Some(DEMO_TO), ReviewStatus::Draft)],
            demo_rules(&first.rule_set_ids[0]),
        );
        let second = replay.calculate("SALE_COSTS_DEMO", base, on, Rounding::HalfUp).unwrap();
        assert_eq!(first.total_cents, second.total_cents);
        assert_eq!(first.lines, second.lines);
    }

    #[test]
    fn a_rule_set_cannot_be_activated_unverified_or_unsigned() {
        let mut set = demo_set("x", DEMO_FROM, None, ReviewStatus::Verif);
        assert!(set.can_activate().is_err());
        set.status = ReviewStatus::Confirmed;
        set.reviewed_by = None;
        assert!(set.can_activate().is_err());
        set.reviewed_by = Some("notary".into());
        assert!(set.can_activate().is_ok());
    }
}
