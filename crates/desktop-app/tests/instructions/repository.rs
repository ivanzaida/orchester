//! Instructions findings/import use the accepted shared bounded, owned-object scanner.
use std::fs;

use orchester_core::{resource::InstructionScope, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{harness::Instructions, support::project_id};

fn directory_link(target: &std::path::Path, link: &std::path::Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link.to_string_lossy().replace('/', "\\"))
      .arg(target.to_string_lossy().replace('/', "\\"))
      .output()
      .expect("private junction");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("private directory link");
}
#[test]
fn instructions_findings_and_import_refuse_external_skill_tree_without_exposing_or_creating_drafts() {
  let mut screen = Instructions::new();
  let outside = screen.root.parent().expect("private fixture").join("private-outside");
  fs::create_dir_all(&outside).expect("outside fixture");
  fs::write(outside.join("SKILL.md"), "PRIVATE outside fixture content").expect("outside file");
  fs::create_dir_all(screen.root.join(".claude")).expect("private parent");
  directory_link(&outside, &screen.root.join(".claude/skills"));
  screen.go();
  screen.until("automatic refusal", |s| s.exists("instructions-repository-error"));
  assert!(!screen.exists("instructions-repository-findings"));
  assert!(!screen.text("screen-instructions").contains("PRIVATE"));
  // The findings' box holds one row with Try again in place of its rows; the rest of the page works.
  let row = screen.text("instructions-repository-error");
  assert!(row.contains("Try again"), "{row}");
  assert!(screen.exists("instructions-empty-new") && screen.exists("instructions-empty-import"));
  screen.click("instructions-repository-error-retry");
  screen.until("read again and refused again", |s| {
    s.exists("instructions-repository-error") && !s.text("instructions-repository-error").contains("Trying again")
  });
  screen.click("instructions-import");
  screen.until("import refusal", |s| s.exists("instruction-import-error"));
  assert!(!screen.exists("instruction-import-create"));
  assert!(!screen.text("screen-instructions").contains("PRIVATE"));
  assert!(screen.listed().entries.is_empty());
  assert_eq!(
    fs::read_to_string(outside.join("SKILL.md")).expect("source"),
    "PRIVATE outside fixture content"
  );
}
#[test]
fn instructions_automatic_findings_and_import_bound_root_instruction_bytes() {
  let mut screen = Instructions::new();
  let source = screen.root.join("AGENTS.md");
  fs::write(&source, vec![b'x'; 4 * 1024 * 1024 + 1]).expect("large private fixture");
  screen.go();
  screen.until("bounded automatic refusal", |s| {
    s.exists("instructions-repository-error")
  });
  screen.click("instructions-import");
  screen.until("bounded import refusal", |s| s.exists("instruction-import-error"));
  assert!(!screen.exists("instruction-import-create"));
  assert!(screen.listed().entries.is_empty());
  assert_eq!(
    fs::metadata(source).expect("source unchanged").len(),
    4 * 1024 * 1024 + 1
  );
}
#[test]
fn instructions_import_stays_drafts_only_with_both_screen_registrations() {
  let mut screen = Instructions::new();
  fs::write(screen.root.join("AGENTS.md"), "Owned instruction").expect("private fixture");
  screen.go();
  screen.click("instructions-import");
  screen.until("owned review", |s| s.exists("instruction-import-create"));
  screen.click("instruction-import-create");
  screen.until("draft created", |s| s.exists("instruction-import-result"));
  let entry = screen.listed().entries[0].clone();
  assert_eq!(entry.body.scope(), &InstructionScope::Project);
  assert_eq!(entry.body.text().as_str(), "Owned instruction");
  assert!(entry.state.has_draft());
  assert!(entry.state.published().is_none());
  let db = ProjectDb::open(&screen.root).expect("private db");
  assert!(
    db.latest_version::<orchester_core::resource::Instruction>(entry.id)
      .expect("versions")
      .is_none()
  );
  assert_eq!(
    fs::read_to_string(screen.root.join("AGENTS.md")).expect("source"),
    "Owned instruction"
  );
}
#[test]
fn instructions_import_with_skills_tells_the_skills_screen() {
  let mut screen = Instructions::new();
  fs::write(screen.root.join("AGENTS.md"), "Keep changes small.").expect("private fixture");
  let skill = screen.root.join(".claude/skills/checks");
  fs::create_dir_all(&skill).expect("private fixture");
  fs::write(skill.join("SKILL.md"), "# Run the checks").expect("private fixture");
  let changes = screen
    .window
    .launch
    .services()
    .expect("the session started")
    .changes
    .subscribe();
  screen.go();
  screen.click("instructions-import");
  screen.until("review", |s| s.exists("instruction-import-create"));
  screen.click("instruction-import-create");
  screen.until("imported", |s| s.exists("instruction-import-result"));
  let counts = changes.borrow().get(&project_id()).copied().unwrap_or_default();
  assert_eq!(counts.instructions, 1);
  assert_eq!(counts.skills, 1);
}
