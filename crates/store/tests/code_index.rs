//! The code index: the project's setting, on until changed, and what each agent invocation got of
//! the index and used before its first edit.

mod support;

use orchester_core::{
  code_index::{CodeIndexSetting, CodeIndexUnavailable, CodeIndexUse},
  id::InvocationId,
  revision::Revision,
  run::{EditProgress, FirstEdit, TokenUsage},
  store::{ActivityStore, SettingsStore},
};
use orchester_store::StoreError;

use self::support::{at, model, started_run};

#[test]
fn code_index_setting_is_on_until_turned_off() {
  let (_repo, mut db, _run, _execution) = started_run();
  assert_eq!(db.code_index().expect("read"), CodeIndexSetting::On);
  db.save_code_index(CodeIndexSetting::Off).expect("save");
  assert_eq!(db.code_index().expect("read"), CodeIndexSetting::Off);
  db.save_code_index(CodeIndexSetting::On).expect("save");
  assert_eq!(db.code_index().expect("read"), CodeIndexSetting::On);
}

#[test]
fn code_index_use_of_an_invocation_round_trips() {
  let (_repo, mut db, run, execution) = started_run();
  let given = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let missing = db
    .start_invocation(run.id(), execution, &model(), at(20))
    .expect("start");
  let revision = Revision::new(&"4".repeat(Revision::LEN)).expect("revision");
  let used = CodeIndexUse::Given { revision };
  db.save_invocation_code_index(given.id, &used).expect("save");
  let unavailable = CodeIndexUse::Unavailable {
    reason: CodeIndexUnavailable::BuildFailed {
      detail: "access is denied".into(),
    },
  };
  db.save_invocation_code_index(missing.id, &unavailable).expect("save");

  assert_eq!(db.invocation_code_index(given.id).expect("read"), Some(used));
  assert_eq!(db.invocation_code_index(missing.id).expect("read"), Some(unavailable));
  let unknown = InvocationId::generate();
  assert_eq!(db.invocation_code_index(unknown).expect("read"), None);
  assert!(matches!(
    db.save_invocation_code_index(unknown, &CodeIndexUse::Off),
    Err(StoreError::InvocationNotFound(_))
  ));
}

#[test]
fn edit_progress_of_an_invocation_is_replaced_as_it_grows() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  assert_eq!(db.edit_progress(invocation.id).expect("read"), None);
  db.save_edit_progress(invocation.id, &EditProgress::default())
    .expect("save");
  let edited = EditProgress {
    before: TokenUsage {
      input: 38_000,
      cached_input: 12_000,
      output: 5_200,
    },
    first_edit: Some(FirstEdit {
      at: at(15),
      path: "src/auth/github.ts".into(),
    }),
  };
  db.save_edit_progress(invocation.id, &edited).expect("save");
  assert_eq!(db.edit_progress(invocation.id).expect("read"), Some(edited));
}
