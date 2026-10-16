//! The cloud's start with a missing or wrong setting: it stops at once and names the environment variable to set,
//! as the operator writes it (`deploy/cloud/README.md`). Runs the built binary; needs no database.

use std::process::{Command, Output};

const REQUIRED: [&str; 6] = [
  "ORCHESTER_CLOUD_PUBLIC_URL",
  "ORCHESTER_CLOUD_DATABASE_URL",
  "ORCHESTER_CLOUD_SMTP_URL",
  "ORCHESTER_CLOUD_SMTP_FROM",
  "ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY",
  "ORCHESTER_CLOUD_ROOT_PUBLIC_KEY",
];

fn value(variable: &str) -> &'static str {
  match variable {
    "ORCHESTER_CLOUD_PUBLIC_URL" => "https://cloud.example.test",
    "ORCHESTER_CLOUD_DATABASE_URL" => "postgres://cloud:pw@127.0.0.1:1/cloud?sslmode=disable",
    "ORCHESTER_CLOUD_SMTP_URL" => "smtp://127.0.0.1:1",
    "ORCHESTER_CLOUD_SMTP_FROM" => "Orchester <no-reply@cloud.example.test>",
    "ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY" => "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
    "ORCHESTER_CLOUD_ROOT_PUBLIC_KEY" => "IKipR35SvHpGcnHNdp2QXlLNBQS0St6caCeRIoyqdZA",
    _ => unreachable!("only the required variables have test values"),
  }
}

/// Runs `orchester-cloud serve` with exactly `settings` in its environment, in a folder without a `cloud.toml`.
fn serve(settings: &[(&str, &str)]) -> Output {
  let mut command = Command::new(env!("CARGO_BIN_EXE_orchester-cloud"));
  command
    .arg("serve")
    .env_clear()
    .current_dir(env!("CARGO_TARGET_TMPDIR"));
  // Windows needs its system folder to start a process with a cleared environment.
  if let Some(root) = std::env::var_os("SystemRoot") {
    command.env("SystemRoot", root);
  }
  command.envs(settings.iter().copied());
  command.output().expect("run orchester-cloud")
}

fn stops_naming(output: &Output, expected: &str) {
  let log = String::from_utf8_lossy(&output.stderr);
  assert!(!output.status.success(), "the cloud started without {expected}");
  assert!(log.contains(expected), "the log does not name {expected}: {log}");
}

#[test]
fn config_a_missing_variable_is_named_in_full() {
  for missing in REQUIRED {
    let settings: Vec<_> = REQUIRED
      .iter()
      .filter(|variable| **variable != missing)
      .map(|variable| (*variable, value(variable)))
      .collect();
    stops_naming(&serve(&settings), &format!("{missing} is required"));
  }
}

#[test]
fn config_a_wrong_value_is_named_in_full() {
  let wrong = [
    ("ORCHESTER_CLOUD_PUBLIC_URL", "https://cloud.example.test/"),
    ("ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY", "not hex"),
    ("ORCHESTER_CLOUD_ROOT_PUBLIC_KEY", "not a key"),
    ("ORCHESTER_CLOUD_SMTP_FROM", "not an address"),
    ("ORCHESTER_CLOUD_SMTP_URL", "http://relay.example.test"),
    ("ORCHESTER_CLOUD_LISTEN", "nowhere"),
    ("ORCHESTER_CLOUD_TRUSTED_PROXIES", "not an address"),
    (
      "ORCHESTER_CLOUD_DATABASE_URL",
      "postgres://cloud:pw@db.example.test/cloud",
    ),
  ];
  for (variable, bad) in wrong {
    let mut settings: Vec<_> = REQUIRED
      .iter()
      .filter(|required| **required != variable)
      .map(|required| (*required, value(required)))
      .collect();
    settings.push((variable, bad));
    stops_naming(&serve(&settings), variable);
  }
}
