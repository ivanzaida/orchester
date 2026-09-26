//! Reading a step's budget. Step budgets are tokens (`docs/design/orchester-workspace.md`, "Token
//! budgets"); published pipeline versions are immutable snapshots, and those published while step
//! budgets were money hold `{"micros": …, "currency": …}`. Money has no token equivalent, so such a
//! step reads as having no step budget — the run budget still bounds it — and the project store
//! keeps the former amount (`money_step_budgets_before_tokens`).

use serde::{Deserialize, Deserializer};

use crate::tokens::TokenBudget;

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
  Tokens(TokenBudget),
  Money {
    #[serde(rename = "micros")]
    _micros: i64,
    #[serde(rename = "currency")]
    _currency: String,
  },
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<TokenBudget>, D::Error> {
  Ok(match Option::<Stored>::deserialize(deserializer)? {
    Some(Stored::Tokens(budget)) => Some(budget),
    Some(Stored::Money { .. }) | None => None,
  })
}
