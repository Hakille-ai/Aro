//! Commercial rights and inference are deliberately separate. Money is integer micro-EUR.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const COMMERCIAL_CATALOG: &str =
    include_str!("../../../packages/contracts/src/commercial-catalog.json");
pub const MICRO_EUR_PER_CENT: i64 = 10_000;
pub const MAX_CREDIT_MICROS: i64 = 1_000_000_000_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommercialPlan {
    #[default]
    Community,
    Cloud,
    Business,
    Enterprise,
}
impl CommercialPlan {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Community => "community",
            Self::Cloud => "cloud",
            Self::Business => "business",
            Self::Enterprise => "enterprise",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingAccount {
    pub plan: CommercialPlan,
    pub status: String,
    pub seats: i32,
    pub valid_until: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub balance_micros: i64,
    pub reserved_micros: i64,
    pub monthly_limit_micros: i64,
    pub per_request_limit_micros: i64,
    pub month_spent_micros: i64,
}
impl Default for BillingAccount {
    fn default() -> Self {
        Self {
            plan: CommercialPlan::Community,
            status: "inactive".into(),
            seats: 1,
            valid_until: None,
            cancel_at_period_end: false,
            balance_micros: 0,
            reserved_micros: 0,
            monthly_limit_micros: 0,
            per_request_limit_micros: 0,
            month_spent_micros: 0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommercialEntitlements {
    pub commercial_use: bool,
    pub managed_sync: bool,
    pub team_administration: bool,
    pub local_models: bool,
    pub byok: bool,
    pub data_export: bool,
}
impl BillingAccount {
    pub fn entitlements(&self, now: DateTime<Utc>) -> CommercialEntitlements {
        let active = self.status == "active" && self.valid_until.is_some_and(|end| end > now);
        let professional = active
            && matches!(
                self.plan,
                CommercialPlan::Business | CommercialPlan::Enterprise
            );
        CommercialEntitlements {
            commercial_use: professional,
            managed_sync: active && self.plan != CommercialPlan::Community,
            team_administration: professional,
            local_models: true,
            byok: true,
            data_export: true,
        }
    }
    pub fn check_reservation(&self, amount: i64) -> Result<(), &'static str> {
        if amount <= 0 || amount > MAX_CREDIT_MICROS {
            return Err("invalid_reservation");
        }
        if amount > self.per_request_limit_micros {
            return Err("request_budget_exceeded");
        }
        let held = self
            .reserved_micros
            .checked_add(amount)
            .ok_or("budget_overflow")?;
        if held > self.balance_micros {
            return Err("insufficient_credit");
        }
        if self
            .month_spent_micros
            .checked_add(held)
            .ok_or("budget_overflow")?
            > self.monthly_limit_micros
        {
            return Err("monthly_budget_exceeded");
        }
        Ok(())
    }
}

/// Rates are micro-EUR per million tokens; round upwards once, after summing both sides.
pub fn inference_charge(input: u64, output: u64, input_rate: u64, output_rate: u64) -> Option<i64> {
    let cost = u128::from(input)
        .checked_mul(u128::from(input_rate))?
        .checked_add(u128::from(output).checked_mul(u128::from(output_rate))?)?;
    let rounded = cost.checked_add(999_999)? / 1_000_000;
    i64::try_from(rounded)
        .ok()
        .filter(|v| *v <= MAX_CREDIT_MICROS)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_subscription_keeps_work_accessible() {
        let account = BillingAccount {
            plan: CommercialPlan::Business,
            status: "active".into(),
            valid_until: Some(Utc::now() - chrono::Duration::seconds(1)),
            ..Default::default()
        };
        let rights = account.entitlements(Utc::now());
        assert!(!rights.commercial_use && !rights.managed_sync);
        assert!(rights.data_export && rights.local_models && rights.byok);
    }
    #[test]
    fn paid_plan_never_bypasses_compute_budget() {
        let account = BillingAccount {
            plan: CommercialPlan::Enterprise,
            ..Default::default()
        };
        assert_eq!(account.check_reservation(1), Err("request_budget_exceeded"));
    }
    #[test]
    fn reservations_count_against_month_and_balance() {
        let mut account = BillingAccount {
            balance_micros: 100,
            reserved_micros: 60,
            monthly_limit_micros: 90,
            month_spent_micros: 10,
            per_request_limit_micros: 100,
            ..Default::default()
        };
        assert!(account.check_reservation(20).is_ok());
        assert_eq!(
            account.check_reservation(21),
            Err("monthly_budget_exceeded")
        );
        account.monthly_limit_micros = 200;
        assert_eq!(account.check_reservation(41), Err("insufficient_credit"));
    }
    #[test]
    fn exact_integer_prices_and_overflow() {
        assert_eq!(inference_charge(1, 1, 100_000, 200_000), Some(1));
        assert_eq!(
            inference_charge(1_000_000, 2_000_000, 100_000, 200_000),
            Some(500_000)
        );
        assert_eq!(
            inference_charge(u64::MAX, u64::MAX, u64::MAX, u64::MAX),
            None
        );
    }
    #[test]
    fn catalog_is_well_formed_and_compute_is_separate() {
        let catalog: serde_json::Value = serde_json::from_str(COMMERCIAL_CATALOG).unwrap();
        assert_eq!(catalog["inferenceIncluded"], false);
        assert_eq!(catalog["plans"].as_array().unwrap().len(), 4);
        assert_eq!(catalog["plans"][2]["amountCents"], 2900);
    }
}
