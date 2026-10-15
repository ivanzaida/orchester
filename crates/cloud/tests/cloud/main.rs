//! Orchester Cloud against a real PostgreSQL, on loopback. This binary runs its own tests so that one PostgreSQL
//! container serves the whole run and is removed at its end, whatever the tests did; each test still creates and
//! drops its own database. `cargo test -p orchester-cloud --test cloud <filter>` runs the tests whose names contain a
//! filter (`::` runs them all). Without a filter it runs them all when `ORCHESTER_CLOUD_TEST_DATABASE_URL` names a
//! PostgreSQL (CI's Linux job), and otherwise nothing, saying so: a plain `cargo test --workspace` (CI's Windows and
//! macOS jobs) has no database. At the end the run checks its whole log for every secret the tests saw.
// A test runner reports on the console.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod accounts;
mod desktop;
mod hosting;
mod identity;
mod limits;
mod sessions;
mod support;

use std::{future::Future, pin::Pin, process::ExitCode};

use support::{Postgres, args::Selection, log::Capture};

type Test = fn() -> Pin<Box<dyn Future<Output = ()> + Send>>;

macro_rules! tests {
  ($($module:ident::$name:ident),* $(,)?) => {
    &[$((concat!(stringify!($module), "::", stringify!($name)), || Box::pin($module::$name()))),*]
  };
}

const TESTS: &[(&str, Test)] = tests![
  accounts::sign_up_confirm_sign_in,
  accounts::unconfirmed_sign_in_is_refused,
  accounts::wrong_password_and_unknown_email_answer_alike,
  accounts::an_earlier_sign_up_cannot_take_the_account,
  accounts::concurrent_confirm_sign_up_and_reset_never_fail,
  accounts::confirm_locks_the_account_before_its_links,
  accounts::a_reset_request_answers_before_any_database_work,
  limits::sign_in_is_rate_limited_per_email_and_address,
  limits::emails_and_link_finishes_are_rate_limited,
  limits::confirm_is_rate_limited_per_link,
  limits::parallel_confirm_guesses_stay_within_the_link_limit,
  sessions::reset_signs_out_every_device,
  sessions::password_change_signs_out_the_other_devices,
  sessions::devices_are_listed_and_revoked,
  sessions::web_sessions_end_twelve_hours_after_sign_in,
  sessions::a_sign_in_racing_a_reset_does_not_survive_it,
  identity::identity_token_verifies_with_the_published_key_set,
  identity::identity_key_rotation_keeps_the_previous_key_then_drops_it,
  identity::many_concurrent_identity_tokens_do_not_starve_the_pool,
  hosting::migrations_apply_once_and_a_newer_database_is_refused,
  hosting::smtp_mailer_sends_through_the_relay,
  hosting::requests_follow_the_contract,
  desktop::desktop_signs_in_and_out_with_a_confirmed_account,
  hosting::unsafe_configuration_is_refused,
];

/// Runs after the tests: what they logged holds none of their passwords, tokens, links, keys or emails.
const LOG_CHECK: &str = "logs::no_secret_in_the_whole_log";

const RUN_ALL: &str = "`cargo test -p orchester-cloud --test cloud ::` (docs/cloud-hosting.md)";

fn main() -> ExitCode {
  let selection = Selection::parse(std::env::args().skip(1));
  if selection.list {
    let names = TESTS.iter().map(|(name, _)| *name).chain([LOG_CHECK]);
    names.for_each(|name| println!("{name}: test"));
    return ExitCode::SUCCESS;
  }
  // A plain `cargo test --workspace` has no PostgreSQL; with the database variable set, or a filter, it runs (review
  // 2, N4).
  let database = std::env::var_os("ORCHESTER_CLOUD_TEST_DATABASE_URL").is_some();
  if selection.unfiltered() && !database {
    println!("running 0 tests: the cloud's tests need PostgreSQL and run only when named, e.g. {RUN_ALL}");
    return ExitCode::SUCCESS;
  }
  let tests: Vec<_> = TESTS.iter().filter(|(name, _)| selection.selects(name)).collect();
  if tests.is_empty() {
    println!("running 0 tests");
    return ExitCode::SUCCESS;
  }
  let capture = Capture::install();
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("test runtime");
  let failed = runtime.block_on(run(&tests, &capture));
  if failed == 0 {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

async fn run(tests: &[&'static (&'static str, Test)], capture: &Capture) -> usize {
  let postgres = match Postgres::start().await {
    Ok(postgres) => support::set_postgres(postgres),
    Err(reason) => {
      eprintln!("error: the cloud's tests need PostgreSQL: {reason}");
      return 1;
    }
  };
  println!("running {} tests", tests.len() + 1);
  let mut running = tokio::task::JoinSet::new();
  for (name, test) in tests {
    let name = *name;
    let test = *test;
    running.spawn(async move { (name, tokio::spawn(test()).await.is_ok()) });
  }
  let mut failed = 0;
  while let Some(finished) = running.join_next().await {
    let (name, passed) = finished.expect("the test runner's task");
    println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
    failed += usize::from(!passed);
  }
  postgres.stop();
  let leaked = check_log(capture);
  println!("test {LOG_CHECK} ... {}", if leaked { "FAILED" } else { "ok" });
  failed += usize::from(leaked);
  println!(
    "\ntest result: {}. {} passed; {failed} failed",
    if failed == 0 { "ok" } else { "FAILED" },
    tests.len() + 1 - failed
  );
  failed
}

/// True when the log holds a secret, or, after a sign-in, holds no request record (then the capture did not work).
fn check_log(capture: &Capture) -> bool {
  let log = capture.text();
  let secrets = support::secrets();
  let mut leaked = false;
  for secret in &secrets {
    if log.contains(secret.as_str()) {
      eprintln!("the log contains a secret of {} characters", secret.len());
      leaked = true;
    }
  }
  let signed_in = secrets.iter().any(|secret| secret.starts_with("orcc_"));
  if signed_in && !(log.contains("route=/api/v1/auth/sessions") && log.contains("status=201")) {
    eprintln!("the log holds no request records; the capture did not work");
    leaked = true;
  }
  leaked
}
