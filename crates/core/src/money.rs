use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoneyError {
  #[error("currency code must be three uppercase ASCII letters, got `{0}`")]
  InvalidCurrency(String),
}

/// ISO 4217 currency code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// An amount in millionths of the currency unit; floating point is never used for money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Money {
  micros: i64,
  currency: Currency,
}

impl Money {
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
