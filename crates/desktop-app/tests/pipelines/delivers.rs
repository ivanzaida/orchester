//! What a pipeline's runs deliver, in Pipeline settings › Delivery (ORC-93): changes until a person chooses otherwise,
//! the choice saved in the draft and kept as the published version's until the next one is published.

use orchester_core::{
  resource::{Deliverable, Pipeline},
  store::ResourceStore,
};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_delivers_is_chosen_in_settings_and_saved_in_the_draft() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-settings");
  editor.until("the settings page", |e| e.exists("pipeline-delivers"));
  assert!(editor.exists("pipeline-delivers-changes"));
  assert!(
    editor
      .text("pipeline-settings-delivery")
      .contains("What a run delivers")
  );
  editor.click("pipeline-delivers-report");
  editor.until("the report choice saved in the draft", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_some_and(|draft| draft.body().delivers() == Deliverable::Report)
  });
  assert_eq!(
    editor
      .db()
      .latest_version::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .delivers(),
    Deliverable::Changes,
    "the published version keeps what it delivers until the next is published"
  );
  assert!(
    editor.text("pipeline-settings-delivery").contains("Changed"),
    "the section says it changed from the published version"
  );
}
