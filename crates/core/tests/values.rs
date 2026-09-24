use std::num::NonZeroU32;

use orchester_core::{
  id::TaskId,
  money::{Currency, Money, MoneyError},
  name::{Name, NameError},
  project::{ProjectKey, TaskKey},
};

#[test]
fn ids_are_time_ordered_and_round_trip_through_text() {
  let first = TaskId::generate();
  let second = TaskId::generate();
  assert!(first < second);
  assert_eq!(first.to_string().parse::<TaskId>().unwrap(), first);
}

#[test]
fn name_is_trimmed_and_bounded() {
  assert_eq!(Name::new("  Add search ").unwrap().as_str(), "Add search");
  assert_eq!(Name::new("   "), Err(NameError::Blank));
  let long = "x".repeat(Name::MAX_CHARS + 1);
  assert_eq!(
    Name::new(&long),
    Err(NameError::TooLong {
      length: Name::MAX_CHARS + 1
    })
  );
}

#[test]
fn project_key_formats_task_key() {
  let key = TaskKey::new(ProjectKey::new("ORC").unwrap(), NonZeroU32::new(129).unwrap());
  assert_eq!(key.to_string(), "ORC-129");
}

#[test]
fn project_key_rejects_invalid_values() {
  for invalid in ["", "O", "orc", "1RC", "OR-C", "ABCDEFGHIJK"] {
    assert!(ProjectKey::new(invalid).is_err(), "{invalid} should be rejected");
  }
  assert!(ProjectKey::new("K2").is_ok());
}

#[test]
fn currency_requires_three_uppercase_letters() {
  assert_eq!(Currency::new("EUR").unwrap().code(), "EUR");
  assert_eq!(Currency::new("usd"), Err(MoneyError::InvalidCurrency("usd".into())));
  assert!(Currency::new("US").is_err());
}

#[test]
fn money_parses_decimal_amounts_exactly() {
  let parse = |text: &str| Money::parse(text, Currency::USD).map(Money::micros);
  assert_eq!(parse("5"), Ok(5_000_000));
  assert_eq!(parse(" 5.5 "), Ok(5_500_000));
  assert_eq!(parse("0.000001"), Ok(1));
  assert_eq!(parse("12."), Ok(12_000_000));
  for invalid in ["", ".5", "-1", "+1", "1,5", "1.0000001", "5 USD", "99999999999999"] {
    assert_eq!(
      parse(invalid),
      Err(MoneyError::InvalidAmount(invalid.into())),
      "{invalid:?} should be rejected"
    );
  }
}

#[test]
fn project_key_is_suggested_from_the_name() {
  let suggest = |name: &str| {
    ProjectKey::suggest(&Name::new(name).expect("valid name"))
      .as_str()
      .to_owned()
  };
  assert_eq!(suggest("Kontur"), "KON");
  assert_eq!(suggest("orchester"), "ORC");
  assert_eq!(suggest("Personal website"), "PW");
  assert_eq!(suggest("my-cool api v2"), "MCAV");
  assert_eq!(suggest("2048 game"), "GAM");
  assert_eq!(suggest("Контур"), "PRJ");
  assert_eq!(suggest("A"), "PRJ");
}
