//! How a turn's failure is kept (ORC-623): every failure, an account problem among them and a compaction that failed
//! on one, is read back as it was written; a row kept before an account problem had a key of its own, its `kind`
//! written beside the failure's, is read as the problem it names.
//! `cargo test -p orchester-core --test orchestrator_turn_failure`.

use std::path::PathBuf;

use orchester_core::{
  connection::AccountStartProblem,
  orchestrator::{ChatEvent, CompactionState, TurnFailure},
};

fn failures() -> Vec<TurnFailure> {
  vec![
    TurnFailure::NotSignedIn,
    TurnFailure::ConnectionGone,
    TurnFailure::Account {
      problem: AccountStartProblem::NoAccount,
    },
    TurnFailure::Account {
      problem: AccountStartProblem::FolderMissing {
        folder: PathBuf::from("accounts/work"),
      },
    },
    TurnFailure::CannotStart {
      reason: "no program".to_owned(),
    },
    TurnFailure::Failed {
      reason: "it broke".to_owned(),
    },
    TurnFailure::Interrupted,
    TurnFailure::TimeLimit { minutes: 10 },
  ]
}

#[test]
fn turn_failure_reads_back_as_written() {
  for failure in failures() {
    let json = serde_json::to_string(&failure).expect("a failure is written");
    let read: TurnFailure = serde_json::from_str(&json).expect(&json);
    assert_eq!(read, failure, "{json}");
  }
}

#[test]
fn turn_failure_keeps_an_account_problem_under_its_own_key() {
  let failure = TurnFailure::Account {
    problem: AccountStartProblem::NoAccount,
  };
  assert_eq!(
    serde_json::to_string(&failure).expect("a failure is written"),
    r#"{"kind":"account","problem":{"kind":"no_account"}}"#
  );
}

#[test]
fn turn_failure_of_a_compaction_reads_back_as_written() {
  for reason in failures() {
    let event = ChatEvent::Compaction {
      before: Some(182_000),
      state: CompactionState::Failed { reason },
    };
    let json = serde_json::to_string(&event).expect("an event is written");
    let read: ChatEvent = serde_json::from_str(&json).expect(&json);
    assert_eq!(read, event, "{json}");
  }
}

#[test]
fn turn_failure_reads_an_account_problem_kept_the_old_way() {
  let no_account: TurnFailure =
    serde_json::from_str(r#"{"kind":"account","kind":"no_account"}"#).expect("the old shape reads");
  assert_eq!(
    no_account,
    TurnFailure::Account {
      problem: AccountStartProblem::NoAccount
    }
  );
  let missing: TurnFailure =
    serde_json::from_str(r#"{"kind":"account","kind":"folder_missing","folder":"accounts/work"}"#)
      .expect("the old shape reads");
  assert_eq!(
    missing,
    TurnFailure::Account {
      problem: AccountStartProblem::FolderMissing {
        folder: PathBuf::from("accounts/work")
      }
    }
  );
}

#[test]
fn turn_failure_of_a_compaction_reads_an_account_problem_kept_the_old_way() {
  let event: ChatEvent = serde_json::from_str(
    r#"{"kind":"compaction","before":182000,"state":{"state":"failed","reason":{"kind":"account","kind":"no_account"}}}"#,
  )
  .expect("the old shape reads");
  assert_eq!(
    event,
    ChatEvent::Compaction {
      before: Some(182_000),
      state: CompactionState::Failed {
        reason: TurnFailure::Account {
          problem: AccountStartProblem::NoAccount
        }
      },
    }
  );
}

#[test]
fn turn_failure_with_two_kinds_of_no_account_problem_is_unreadable() {
  for json in [
    r#"{"kind":"failed","kind":"no_account","reason":"x"}"#,
    r#"{"kind":"account","kind":"no_account","kind":"no_account"}"#,
    r#"{"kind":"account","kind":"no_account","problem":{"kind":"no_account"}}"#,
  ] {
    assert!(serde_json::from_str::<TurnFailure>(json).is_err(), "{json}");
  }
}
