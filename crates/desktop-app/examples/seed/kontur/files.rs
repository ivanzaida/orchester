//! The files of the QA repository: a small webhook service, as the design's Kontur project shows it, and
//! the work the design shows on KON-12 (Add retry to webhook sender): four files in its checkpoint and two
//! more in its worktree since (`Task · Changes · dark` `DYun5`).

/// The repository's first commit.
pub fn initial() -> Vec<(&'static str, String)> {
  vec![
    ("README.md", README.to_owned()),
    ("Cargo.toml", CARGO.to_owned()),
    ("src/lib.rs", "pub mod webhooks;\n".to_owned()),
    (
      "src/webhooks/mod.rs",
      "pub mod log;\npub mod sender;\npub mod settings;\n".to_owned(),
    ),
    ("src/webhooks/sender.rs", SENDER.to_owned()),
    ("src/webhooks/settings.rs", SETTINGS.to_owned()),
    ("src/webhooks/log.rs", LOG.to_owned()),
    ("tests/webhooks/retry.rs", RETRY_TEST.to_owned()),
    ("migrations/0041_endpoints.sql", ENDPOINTS_SQL.to_owned()),
  ]
}

/// KON-12's checkpoint after Implement.
pub fn retry_checkpoint() -> Vec<(&'static str, String)> {
  vec![
    ("src/webhooks/sender.rs", SENDER.replace(SENDER_BODY, RETRY_BODY)),
    (
      "src/webhooks/settings.rs",
      SETTINGS.replace(
        "  pub timeout_secs: u64,\n",
        "  pub timeout_secs: u64,\n  /// Attempts per delivery, the first included.\n  pub retry_budget: u32,\n",
      ),
    ),
    ("migrations/0042_delivery_attempts.sql", ATTEMPTS_SQL.to_owned()),
    (
      "tests/webhooks/retry.rs",
      format!(
        "{RETRY_TEST}\n#[test]\nfn the_default_budget_is_five_attempts() {{\n  assert_eq!(Settings::default().retry_budget, 5);\n}}\n"
      ),
    ),
  ]
}

/// What KON-12's Builder changed in its worktree since that checkpoint.
pub fn retry_in_progress() -> Vec<(&'static str, String)> {
  vec![
    (
      "src/webhooks/log.rs",
      LOG.replace(
        "  pub fn record(&mut self, endpoint: &str, status: u16) {\n",
        "  pub fn record(&mut self, endpoint: &str, attempt: u32, status: u16) {\n    self.attempts.push(attempt);\n",
      ),
    ),
    ("tests/webhooks/jitter.rs", JITTER_TEST.to_owned()),
  ]
}

const README: &str = "# Kontur\n\nDelivers webhooks, exports invoices and imports contacts.\n\n\
                      Run the tests with `cargo test`.\n";

const CARGO: &str = "[package]\nname = \"kontur\"\nversion = \"0.4.0\"\nedition = \"2024\"\n";

const SENDER: &str = r"use crate::webhooks::{log::DeliveryLog, settings::Endpoint};

pub struct WebhookSender {
  client: Client,
  log: DeliveryLog,
}

impl WebhookSender {
  pub async fn send(&self, endpoint: &Endpoint, event: &Event) -> Result<Delivery, SendError> {
    let body = serde_json::to_vec(event)?;
    let response = self.client.post(&endpoint.url).body(body).send().await?;
    Ok(Delivery::from_response(response))
  }
}

fn is_retryable(status: StatusCode) -> bool {
  status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}
";

const SENDER_BODY: &str = "    let response = self.client.post(&endpoint.url).body(body).send().await?;
    Ok(Delivery::from_response(response))
";

const RETRY_BODY: &str = "    let budget = endpoint.settings.retry_budget;
    let mut attempt = 0;
    loop {
      attempt += 1;
      let started = Instant::now();
      match self.client.post(&endpoint.url).body(body.clone()).send().await {
        Ok(response) if response.status().is_success() => {
          self.log.record(endpoint, attempt, response.status(), started.elapsed());
          return Ok(Delivery::from_response(response));
        }
        Ok(response) if attempt < budget && is_retryable(response.status()) => {
          let delay = retry_after(&response).unwrap_or_else(|| backoff(attempt));
          sleep(delay.min(MAX_RETRY_DELAY)).await;
        }
        Ok(response) => return Err(SendError::Status(response.status())),
        Err(err) if attempt < budget => sleep(backoff(attempt)).await,
        Err(err) => return Err(SendError::Network(err)),
      }
    }
";

const SETTINGS: &str = r"#[derive(Clone, Debug)]
pub struct Settings {
  pub url: String,
  pub timeout_secs: u64,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      url: String::new(),
      timeout_secs: 10,
    }
  }
}
";

const LOG: &str = r"#[derive(Default)]
pub struct DeliveryLog {
  statuses: Vec<u16>,
}

impl DeliveryLog {
  pub fn record(&mut self, endpoint: &str, status: u16) {
    self.statuses.push(status);
  }
}
";

const RETRY_TEST: &str = r"use kontur::webhooks::settings::Settings;

#[test]
fn a_new_endpoint_times_out_after_ten_seconds() {
  assert_eq!(Settings::default().timeout_secs, 10);
}
";

const JITTER_TEST: &str = r"use kontur::webhooks::sender::backoff;

#[test]
fn backoff_stays_under_the_jitter_bound() {
  for attempt in 1..10 {
    assert!(backoff(attempt).as_millis() <= 500 * 2u128.pow(attempt));
  }
}
";

const ENDPOINTS_SQL: &str = "CREATE TABLE endpoints (\n  id INTEGER PRIMARY KEY,\n  url TEXT NOT NULL\n);\n";

const ATTEMPTS_SQL: &str = "CREATE TABLE delivery_attempts (\n  id INTEGER PRIMARY KEY,\n  endpoint_id INTEGER NOT NULL \
                            REFERENCES endpoints (id),\n  attempt INTEGER NOT NULL,\n  status INTEGER,\n  delay_ms \
                            INTEGER NOT NULL\n);\n";
