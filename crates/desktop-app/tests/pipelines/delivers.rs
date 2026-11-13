//! What a pipeline's runs deliver, in Pipeline settings › Delivery (ORC-93): changes until a person chooses otherwise,
//! the choice saved as the next version while the version before keeps its own (a pipeline holds no draft, D-032).

use orchester_core::resource::Deliverable;

use crate::pipelines_support::Editor;

#[test]
fn pipelines_delivers_is_chosen_in_settings_and_saved_as_the_next_version() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-settings");
  editor.until("the Settings tab", |e| e.exists("pipeline-delivers-changes"));
  assert!(
    editor
      .text("pipeline-settings-delivery")
      .contains("What a run that passes every step leaves"),
    "the section says what a run delivers"
  );
  assert!(
    editor
      .text("pipeline-delivers-report")
      .contains("no repository changes are expected"),
    "each choice says what it means"
  );
  editor.click("pipeline-delivers-report");
  assert_eq!(editor.version(id), 1, "nothing saves on its own");
  editor.click("pipeline-save");
  editor.until("the report choice saved as v2", |e| {
    e.version(id) == 2 && e.stored(id).delivers() == Deliverable::Report
  });
  let versions = editor.versions(id);
  let first = versions.first().expect("v1");
  assert_eq!(
    first.delivers(),
    Deliverable::Changes,
    "the version before keeps what it delivers"
  );
}
