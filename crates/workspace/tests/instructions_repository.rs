//! Instructions-specific import naming stays in the workspace and persists unchanged.
use orchester_core::{agent_files::import::Candidate, id::IdempotencyKey};
use orchester_workspace::instructions::{data::ProjectInstructions, repository};

#[test]
fn instructions_repository_names_drafts_from_stems_and_preserves_source_paths() {
  let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/instructions-import");
  std::fs::create_dir_all(&scratch).expect("private scratch");
  let dir = tempfile::tempdir_in(scratch).expect("private project");
  std::fs::create_dir(dir.path().join(".git")).expect("private git repository");
  for file in ["AGENTS.md", "CLAUDE.md"] {
    std::fs::write(dir.path().join(file), "Keep changes small.").expect("private instructions");
  }
  let scan = repository::scan(dir.path()).expect("scan");
  for (item, stem) in scan.items.iter().zip(["AGENTS", "CLAUDE"]) {
    let Candidate::Instruction { name, .. } = item.outcome.as_ref().expect("candidate") else {
      panic!("instruction candidate");
    };
    assert_eq!(name.as_str(), stem);
    assert_eq!(item.source, format!("{stem}.md"));
  }
  // The project store normally exists before the import page is opened.
  orchester_store::ProjectDb::create(
    dir.path(),
    orchester_core::project::Project::new(
      orchester_core::id::ProjectId::generate(),
      orchester_core::project::ProjectKey::new("IMP").expect("key"),
      orchester_core::name::Name::new("Import fixture").expect("name"),
    ),
  )
  .expect("project");
  let made = repository::create_drafts_in(dir.path(), IdempotencyKey::generate(), &scan.items).expect("import");
  assert_eq!(made[0].source, "AGENTS.md");
  let data = ProjectInstructions::load_in(dir.path()).expect("instructions");
  assert!(
    data
      .entries
      .iter()
      .any(|entry| entry.name.as_ref() == "AGENTS" && entry.state.has_draft())
  );
  assert!(data.entries.iter().all(|entry| entry.state.published().is_none()));
}
