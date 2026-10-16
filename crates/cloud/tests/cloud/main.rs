//! Orchester Cloud against a real PostgreSQL, on loopback. This binary runs its own tests so that one PostgreSQL
//! container serves the whole run and is removed at its end, whatever the tests did; each test still creates and
//! drops its own database. `cargo test -p orchester-cloud --test cloud <filter>` runs the tests whose names contain a
//! filter (`::` runs them all). Without a filter it runs them all when `ORCHESTER_CLOUD_TEST_DATABASE_URL` names a
//! PostgreSQL (CI's Linux job), and otherwise nothing, saying so: a plain `cargo test --workspace` (CI's Windows and
//! macOS jobs) has no database. At the end the run checks its whole log for every secret the tests saw.
// A test runner reports on the console.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod accept;
mod accounts;
mod caps;
mod deletion;
mod desktop;
mod ended;
mod hosting;
mod identity;
mod limits;
mod links;
mod registry;
mod root_key;
mod sessions;
mod support;
mod two_factor;
mod two_factor_guards;
mod two_factor_sign_in;

use std::{future::Future, pin::Pin, process::ExitCode, sync::Arc};

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
  links::a_resent_link_confirms_with_the_sign_up_password,
  links::resend_answers_alike_for_any_email,
  links::resend_answers_before_the_database_and_is_limited,
  links::inspect_names_the_email_without_using_the_link,
  links::inspect_failures_are_limited_per_address,
  limits::sign_in_is_rate_limited_per_email_and_address,
  limits::emails_and_link_finishes_are_rate_limited,
  limits::confirm_is_rate_limited_per_link,
  limits::parallel_confirm_guesses_stay_within_the_link_limit,
  deletion::deleting_the_account_removes_what_the_cloud_holds,
  deletion::deleting_with_wrong_passwords_is_limited,
  deletion::a_deleted_owners_server_stays_for_its_members,
  deletion::answered_invites_go_with_the_account_and_ownerless_servers_still_issue,
  caps::a_person_keeps_their_caps_past_deleting_the_account,
  sessions::reset_signs_out_every_device,
  sessions::password_change_signs_out_the_other_devices,
  sessions::devices_are_listed_and_revoked,
  sessions::web_sessions_end_twelve_hours_after_sign_in,
  sessions::a_sign_in_racing_a_reset_does_not_survive_it,
  ended::a_refused_token_says_why_it_ended,
  ended::tokens_of_a_deleted_account_say_so_until_they_would_have_ended,
  ended::tokens_ended_before_the_account_was_deleted_say_nothing,
  identity::identity_token_verifies_with_the_published_key_set,
  identity::identity_key_rotation_keeps_the_previous_key_then_drops_it,
  identity::identity_key_sets_are_signed_weekly_and_renewed_after_a_stop,
  identity::identity_keys_leave_no_gap_after_a_stop,
  identity::many_concurrent_identity_tokens_do_not_starve_the_pool,
  hosting::migrations_apply_once_and_a_newer_database_is_refused,
  hosting::migration_4_removes_servers_whose_id_is_not_their_keys,
  hosting::smtp_mailer_sends_through_the_relay,
  hosting::requests_follow_the_contract,
  desktop::desktop_signs_in_and_out_with_a_confirmed_account,
  hosting::unsafe_configuration_is_refused,
  registry::servers_link_from_the_statement_they_signed,
  registry::a_server_id_is_its_own_keys,
  registry::a_statement_works_once_and_a_newer_one_takes_the_server_back,
  registry::an_account_owns_at_most_five_servers,
  registry::a_revoked_invite_cannot_be_delivered_again,
  registry::invite_emails_have_their_own_buckets,
  registry::invite_deliveries_are_limited_per_address_and_per_owner,
  registry::invite_ids_are_one_servers_own,
  registry::admin_tokens_are_issued_only_for_linked_servers,
  registry::invites_reach_the_invitee_once_and_end_when_revoked,
  registry::invites_are_declined_by_the_invitee,
  accept::invites_are_accepted_with_a_token_that_names_them,
  root_key::the_first_start_makes_the_root,
  root_key::a_restart_reuses_the_root,
  root_key::replicas_starting_together_make_one_root,
  root_key::a_changed_key_encryption_key_stops_the_start,
  root_key::keys_without_a_root_stop_the_start,
  root_key::losing_the_key_encryption_key_starts_over_with_a_new_root,
  root_key::a_corrupt_root_is_stored_data,
  root_key::a_key_set_dated_in_the_future_is_ignored,
  root_key::keys_verify_with_the_root_a_linked_server_pins,
  two_factor::turning_it_on_takes_the_password_and_a_code_and_upgrades_that_session,
  two_factor::a_code_works_once_within_its_window,
  two_factor::recovery_codes_work_once_and_are_replaced_with_a_code,
  two_factor::turning_it_off_takes_the_password_and_a_code,
  two_factor_sign_in::the_password_step_answers_a_challenge_that_one_code_finishes,
  two_factor_sign_in::code_tries_are_limited_per_challenge_and_per_account,
  two_factor_sign_in::a_password_change_ends_a_challenge,
  two_factor_sign_in::invites_need_a_two_factor_session_and_tokens_say_so,
  two_factor_sign_in::an_operator_turns_it_off_with_the_documented_statement,
  two_factor_sign_in::tokens_for_codes_name_their_use_and_the_certificate_asked_for,
  two_factor_guards::wrong_codes_are_capped_a_day_and_the_tenth_is_mailed,
  two_factor_guards::a_code_is_spent_once_by_requests_that_race,
  two_factor_guards::a_password_alone_changes_nothing_sensitive_while_it_is_on,
];

/// Tests that run at once: each holds a few connections, some many more, and PostgreSQL allows 100 by default (CI's
/// service container keeps the default).
const PARALLEL: usize = 16;

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
  let slots = Arc::new(tokio::sync::Semaphore::new(PARALLEL));
  for (name, test) in tests {
    let name = *name;
    let test = *test;
    let slots = Arc::clone(&slots);
    running.spawn(async move {
      let _slot = slots.acquire_owned().await.expect("the runner never closes its slots");
      (name, tokio::spawn(test()).await.is_ok())
    });
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
