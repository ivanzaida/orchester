use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoneyError {
  #[error("currency code must be three uppercase ASCII letters, got `{0}`")]
  InvalidCurrency(String),
  #[error(
    "amount must be digits with at most {places} decimal places, got `{0}`",
    places = Money::DECIMAL_PLACES
  )]
  InvalidAmount(String),
}

/// ISO 4217 currency code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency([u8; 3]);

impl Currency {
  pub const USD: Self = Self(*b"USD");

  pub fn new(code: &str) -> Result<Self, MoneyError> {
    match code.as_bytes() {
      &[a, b, c] if [a, b, c].iter().all(u8::is_ascii_uppercase) => Ok(Self([a, b, c])),
      _ => Err(MoneyError::InvalidCurrency(code.to_owned())),
    }
  }

  pub fn code(&self) -> &str {
    std::str::from_utf8(&self.0).expect("currency codes are validated as ASCII")
  }
}

impl TryFrom<String> for Currency {
  type Error = MoneyError;

  fn try_from(code: String) -> Result<Self, MoneyError> {
    Self::new(&code)
  }
}

impl From<Currency> for String {
  fn from(currency: Currency) -> Self {
    currency.code().to_owned()
  }
}

/// An amount in millionths of the currency unit; floating point is never used for money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
  micros: i64,
  currency: Currency,
}

impl Money {
  /// Decimal places an amount can carry: one micro-unit is the smallest step.
  pub const DECIMAL_PLACES: usize = 6;

  /// Reads a non-negative decimal amount such as `5`, `5.5` or `0.25`, as a person types it.
  /// Signs, separators and more than [`Self::DECIMAL_PLACES`] decimals are rejected rather
  /// than rounded, so the stored amount is exactly what was entered.
  pub fn parse(amount: &str, currency: Currency) -> Result<Self, MoneyError> {
    let invalid = || MoneyError::InvalidAmount(amount.to_owned());
    let trimmed = amount.trim();
    let (units, fraction) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    let digits_only = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if units.is_empty() || !digits_only(units) || !digits_only(fraction) || fraction.len() > Self::DECIMAL_PLACES {
      return Err(invalid());
    }
    let padded = format!("{fraction:0<width$}", width = Self::DECIMAL_PLACES);
    let units: i64 = units.parse().map_err(|_| invalid())?;
    let fraction: i64 = padded.parse().map_err(|_| invalid())?;
    let micros = units
      .checked_mul(1_000_000)
      .and_then(|micros| micros.checked_add(fraction))
      .ok_or_else(invalid)?;
    Ok(Self { micros, currency })
  }

  pub const fn from_micros(micros: i64, currency: Currency) -> Self {
    Self { micros, currency }
  }

  pub const fn micros(self) -> i64 {
    self.micros
  }

  pub const fn currency(self) -> Currency {
    self.currency
  }
}
