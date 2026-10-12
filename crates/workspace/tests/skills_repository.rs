//! Discovery owns its reads; all external/protected targets here are disposable private fixtures.
use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  agent_files::import::Candidate,
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::Skill,
  store::{BlobStore, ResourceStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::skills::{
  data,
  repository::{self, ScanError},
};
struct Fixture {
  _dir: tempfile::TempDir,
  root: PathBuf,
  outside: PathBuf,
}
impl Fixture {
  fn new() -> Self {
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/skills-expanded/fixtures");
    fs::create_dir_all(&scratch).expect("scratch");
    let resolved = fs::canonicalize(&scratch).expect("canonical scratch");
    let text = resolved.to_string_lossy();
    let scratch = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text));
    let dir = tempfile::tempdir_in(scratch).expect("fixture");
    let root = dir.path().join("project");
    let outside = dir.path().join("outside");
    fs::create_dir_all(root.join(".git")).expect("git");
    fs::create_dir_all(&outside).expect("outside");
    ProjectDb::create(
      &root,
      Project::new(
        ProjectId::generate(),
        ProjectKey::new("SKL").expect("key"),
        Name::new("Private fixture").expect("name"),
      ),
    )
    .expect("db");
    Self {
      _dir: dir,
      root,
      outside,
    }
  }
  fn skill(&self) -> PathBuf {
    let path = self.root.join(".claude/skills/checks");
    write(&path.join("SKILL.md"), "# Safe instructions");
    path
  }
  fn refusal(&self) -> ScanError {
    let error = repository::scan(&self.root).expect_err("discovery refuses");
    assert!(
      data::ProjectSkills::load_in(&self.root)
        .expect("skills")
        .entries
        .is_empty(),
      "a rejected scan cannot expose candidates or produce imported blobs"
    );
    error
  }
}
fn write(path: &Path, content: &str) {
  fs::create_dir_all(path.parent().expect("parent")).expect("directory");
  fs::write(path, content).expect("fixture file");
}
fn directory_link(target: &Path, link: &Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link.to_string_lossy().replace('/', "\\"))
      .arg(target.to_string_lossy().replace('/', "\\"))
      .output()
      .expect("junction");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("directory link");
}
fn file_link(target: &Path, link: &Path) {
  #[cfg(windows)]
  std::os::windows::fs::symlink_file(target, link).expect("fixture file symlink");
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("fixture file symlink");
}
#[test]
fn skills_repository_refuses_external_directories_before_traversal() {
  for relative in [".claude/skills", ".claude/skills/escaped"] {
    let f = Fixture::new();
    write(&f.outside.join("SKILL.md"), "PRIVATE FIXTURE: never discover this");
    fs::create_dir_all(f.root.join(relative).parent().expect("parent")).expect("parent");
    directory_link(&f.outside, &f.root.join(relative));
    assert!(matches!(f.refusal(), ScanError::Outside(_)));
  }
}
#[test]
fn skills_repository_refuses_external_skill_reference_and_agent_file_links() {
  for relative in [
    ".claude/skills/checks/SKILL.md",
    ".claude/skills/checks/notes.md",
    "AGENTS.md",
  ] {
    let f = Fixture::new();
    let skill = f.skill();
    if relative.ends_with("SKILL.md") {
      fs::remove_file(skill.join("SKILL.md")).expect("replace fixture");
    }
    let target = f.outside.join("private.md");
    write(&target, "PRIVATE FIXTURE: not repository content");
    file_link(&target, &f.root.join(relative));
    assert!(matches!(f.refusal(), ScanError::Outside(_)));
  }
}
#[test]
fn skills_repository_refuses_protected_targets_and_other_skill_roots() {
  for relative in [".git/private", ".orchester/private", ".claude/skills/other"] {
    let f = Fixture::new();
    let skill = f.skill();
    let target = f.root.join(relative).join("private.md");
    write(&target, "PROTECTED FIXTURE");
    file_link(&target, &skill.join("notes.md"));
    assert!(matches!(f.refusal(), ScanError::Protected(_) | ScanError::Outside(_)));
  }
}
#[test]
fn skills_repository_refuses_protected_directory_targets() {
  for relative in [".git/private", ".orchester/private"] {
    let f = Fixture::new();
    f.skill();
    write(&f.root.join(relative).join("SKILL.md"), "PROTECTED FIXTURE");
    directory_link(&f.root.join(relative), &f.root.join(".claude/skills/escaped"));
    assert!(matches!(f.refusal(), ScanError::Protected(_)));
  }
}
#[test]
fn skills_repository_accepts_in_project_junctions_and_explicit_picker_without_source_writes() {
  let f = Fixture::new();
  let skill = f.skill();
  let notes = skill.join("original.md");
  write(&notes, "Allowed private reference fixture");
  fs::create_dir_all(f.root.join(".agents")).expect("agents");
  directory_link(&f.root.join(".claude/skills"), &f.root.join(".agents/skills"));
  let scan = repository::scan(&f.root).expect("contained discovery");
  assert_eq!(scan.found().1, 2);
  assert!(
    scan
      .items
      .iter()
      .all(|i| matches!(&i.outcome, Ok(Candidate::Skill { files, .. }) if files.len() == 1))
  );
  let created = repository::create_drafts_in(&f.root, IdempotencyKey::generate(), &scan.items).expect("import");
  assert_eq!(created.len(), 2);
  let db = ProjectDb::open(&f.root).expect("db");
  let body = db
    .draft::<Skill>(created[0].resource.id())
    .expect("draft")
    .expect("imported");
  assert_eq!(
    db.blob(&body.body().files()[0].content).expect("blob"),
    b"Allowed private reference fixture"
  );
  assert_eq!(
    fs::read_to_string(notes).expect("source unchanged"),
    "Allowed private reference fixture"
  );
  assert_eq!(
    fs::read_to_string(skill.join("SKILL.md")).expect("source unchanged"),
    "# Safe instructions"
  );
  let picked = f.outside.join("chosen.md");
  write(&picked, "Explicitly selected external reference");
  let selected = data::add_file_in(&f.root, &picked).expect("explicit picker operation is distinct");
  assert_eq!(
    db.blob(&selected.content).expect("blob"),
    b"Explicitly selected external reference"
  );
}
#[test]
fn skills_repository_bounds_file_size_total_bytes_depth_and_entry_count() {
  let f = Fixture::new();
  write(&f.skill().join("large.md"), &"x".repeat(4 * 1024 * 1024 + 1));
  assert!(matches!(f.refusal(), ScanError::Bounds(_)));
  let f = Fixture::new();
  let skill = f.skill();
  for n in 0..5 {
    write(&skill.join(format!("file{n}.md")), &"x".repeat(4 * 1024 * 1024));
  }
  assert!(matches!(f.refusal(), ScanError::Bounds(_)));
  let f = Fixture::new();
  fs::create_dir_all(f.skill().join("a/b/c/d/e/f/g/h/i")).expect("deep");
  assert!(matches!(f.refusal(), ScanError::Bounds(_)));
  let f = Fixture::new();
  let skill = f.skill();
  for n in 0..4097 {
    write(&skill.join(format!("file{n}.md")), "");
  }
  assert!(matches!(f.refusal(), ScanError::Bounds(_)));
}
#[test]
fn skills_repository_refuses_nonregular_skill_and_instruction_files() {
  for relative in [".claude/skills/checks/SKILL.md", "AGENTS.md"] {
    let f = Fixture::new();
    fs::create_dir_all(f.root.join(relative)).expect("nonregular directory");
    assert!(matches!(f.refusal(), ScanError::NotRegular(_)));
  }
}
#[cfg(unix)]
#[test]
fn skills_repository_never_opens_a_special_file() {
  for relative in ["SKILL.md", "notes.md"] {
    let f = Fixture::new();
    let skill = f.skill();
    let path = skill.join(relative);
    if path.exists() {
      fs::remove_file(&path).expect("replace fixture");
    }
    let made = std::process::Command::new("mkfifo")
      .arg(&path)
      .status()
      .expect("mkfifo");
    assert!(made.success());
    assert!(matches!(f.refusal(), ScanError::NotRegular(_)));
  }
}
#[cfg(windows)]
#[test]
fn skills_repository_never_opens_a_special_file() {
  let f = Fixture::new();
  let skill = f.skill();
  file_link(Path::new(r"\\.\NUL"), &skill.join("notes.md"));
  assert!(matches!(
    f.refusal(),
    ScanError::Outside(_) | ScanError::Read { .. } | ScanError::NotRegular(_)
  ));
}

#[test]
fn skills_repository_accepts_in_project_reference_file_links() {
  let f = Fixture::new();
  let skill = f.skill();
  let notes = skill.join("original.md");
  write(&notes, "Allowed linked reference");
  file_link(&notes, &skill.join("notes.md"));
  let scan = repository::scan(&f.root).expect("allowed file link");
  assert!(
    matches!(&scan.items[0].outcome, Ok(Candidate::Skill { files, .. }) if files.len() == 2
    && files.iter().all(|file| file.content.as_ref() == b"Allowed linked reference"))
  );
}
