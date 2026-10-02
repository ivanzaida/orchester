//! Skills (ORC-15): the empty library with what the repository gives its agents, Import from
//! repository (its choices, Enter to create the drafts, the result, a repository left as it was),
//! a skill written and published in its editor, and Repository agent files in Project settings.

use std::{fs, path::Path};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  resource::{Instruction, ResourceKind, Skill},
  store::{ResourceStore, SettingsStore},
};
use orchester_store::ProjectDb;

use crate::{
  agent_permissions::shows,
  harness::{Desktop, expected},
};

const AGENTS_MD: &str = "# Kontur\n\nUse {{task.id}} as written.\n";
const RUN_CHECKS: &str = "---\nname: run-checks\ndescription: Which pnpm scripts to run and when.\n---\n\nRun them.\n";

/// AGENTS.md, a CLAUDE.md that only imports it, a skill with a reference file and a skills folder
/// without a SKILL.md.
fn repository_files(repo: &Path) {
  let write = |path: &str, text: &str| {
    let path = repo.join(path);
    fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
    fs::write(path, text).expect("file");
  };
  write("AGENTS.md", AGENTS_MD);
  write("CLAUDE.md", "@AGENTS.md\n");
  write(".claude/skills/run-checks/SKILL.md", RUN_CHECKS);
  write(".claude/skills/run-checks/reference/ci-order.txt", "lint\ntest\n");
  write(".agents/skills/old-deploy/notes.md", "old\n");
}

/// Every file of the repository outside `.git` and `.orchester`, with its content.
fn snapshot(repo: &Path) -> Vec<(String, Vec<u8>)> {
  fn walk(root: &Path, folder: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(folder).expect("folder") {
      let path = entry.expect("entry").path();
      let name = path.file_name().expect("name").to_string_lossy().into_owned();
      if name == ".git" || name == ".orchester" {
        continue;
      }
      if path.is_dir() {
        walk(root, &path, files);
      } else {
        let relative = path.strip_prefix(root).expect("inside").to_string_lossy().into_owned();
        files.push((relative, fs::read(&path).expect("file")));
      }
    }
  }
  let mut files = Vec::new();
  walk(repo, repo, &mut files);
  files.sort();
  files
}

fn open_skills(desktop: &mut Desktop) {
  desktop.wait_for("nav-skills");
  desktop.click("nav-skills");
  desktop.wait_for("skills-library");
}

#[test]
fn skills_empty_library_shows_what_the_repository_gives_its_agents() {
  let mut desktop = Desktop::with_seeded_project(repository_files);
  open_skills(&mut desktop);
  desktop.wait_for("skills-found");
  assert!(shows(&mut desktop, "No skills in Kontur yet"));
  assert!(shows(&mut desktop, "becomes a draft project instruction"));
  assert!(shows(
    &mut desktop,
    "Only points to AGENTS.md (@AGENTS.md) · nothing to import"
  ));
  assert!(shows(&mut desktop, "becomes draft skill run-checks · 1 file"));
  assert!(shows(&mut desktop, "No SKILL.md · nothing to import"));
  // The library is outside any form: its actions are pointer-only.
  assert_eq!(desktop.stops(1, false), expected(&[None]));
}

#[test]
fn skills_import_creates_drafts_and_leaves_the_repository_unchanged() {
  let mut desktop = Desktop::with_seeded_project(repository_files);
  let before = snapshot(desktop.repo());
  open_skills(&mut desktop);
  desktop.click("skills-import");
  // The dialog opens while it reads the repository; the choice is shown once it is read.
  desktop.wait_until("the repository read", |desktop| {
    shows(desktop, "2 of 4 selected · 1 instruction, 1 skill")
  });
  // The checks of what can be imported, Cancel and Create; CLAUDE.md and the folder without a
  // SKILL.md cannot be chosen.
  assert_eq!(
    desktop.stops(5, false),
    expected(&[
      Some("repository-import-check-AGENTS.md"),
      Some("repository-import-check-.claude/skills/run-checks"),
      Some("repository-import-cancel"),
      Some("repository-import-create"),
      Some("repository-import-check-AGENTS.md"),
    ])
  );
  desktop.tab_to("repository-import-create", 4);
  desktop.enter();
  desktop.settle("the drafts created", |tree| {
    tree
      .get_element_by_id("repository-import-title")
      .and_then(|title| title.text_content().map(str::to_owned))
      .is_some_and(|title| title == "2 drafts created")
  });
  assert!(shows(&mut desktop, "CLAUDE.md only points to AGENTS.md."));
  let db = ProjectDb::open(desktop.repo()).expect("project");
  let instruction = db.resources(ResourceKind::Instruction).expect("list").remove(0);
  let draft = db
    .draft::<Instruction>(instruction.id())
    .expect("read")
    .expect("a draft");
  assert_eq!(draft.body().text().render(|_| None).expect("no variables"), AGENTS_MD);
  assert_eq!(db.latest_version::<Instruction>(instruction.id()).expect("read"), None);
  let skill = db.resources(ResourceKind::Skill).expect("list").remove(0);
  assert_eq!(skill.slug().as_str(), "run-checks");
  let body = db.draft::<Skill>(skill.id()).expect("read").expect("a draft");
  assert_eq!(body.body().description(), "Which pnpm scripts to run and when.");
  assert_eq!(body.body().files()[0].path.as_str(), "reference/ci-order.txt");
  assert_eq!(snapshot(desktop.repo()), before, "the repository is only read");
}

/// The skill `name`'s draft description and whether it has a published version.
fn stored(repo: &Path) -> Option<(String, bool)> {
  let db = ProjectDb::open(repo).ok()?;
  let skill = db.resources(ResourceKind::Skill).ok()?.into_iter().next()?;
  let published = db.latest_version::<Skill>(skill.id()).ok()?.is_some();
  let description = match db.draft::<Skill>(skill.id()).ok()? {
    Some(draft) => draft.body().description().to_owned(),
    None => db
      .latest_version::<Skill>(skill.id())
      .ok()??
      .body()
      .description()
      .to_owned(),
  };
  Some((description, published))
}

#[test]
fn skills_new_skill_is_written_saved_and_published() {
  let mut desktop = Desktop::with_project();
  open_skills(&mut desktop);
  desktop.click("skills-new");
  desktop.wait_for("new-skill-name");
  desktop.click("new-skill-name");
  desktop.type_text("run-checks");
  desktop.enter();
  desktop.wait_for("skill-description");
  let repo = desktop.repo().to_path_buf();
  desktop.click("skill-description");
  desktop.type_text("Runs checks.");
  desktop.click("skill-body");
  desktop.type_text("Run them.");
  desktop.settle("the draft saved", |_| {
    stored(&repo).is_some_and(|(description, _)| description == "Runs checks.")
  });
  desktop.settle("the save state shown", |tree| {
    tree
      .get_element_by_id("skill-save-status")
      .and_then(|status| status.text_content().map(str::to_owned))
      .is_some_and(|status| status == "Draft saved · Not published")
  });
  desktop.click("skill-publish");
  desktop.settle("v1 published", |_| {
    stored(&repo).is_some_and(|(_, published)| published)
  });
}

#[test]
fn skills_setting_saves_when_the_switch_flips() {
  let mut desktop = Desktop::with_seeded_project(repository_files);
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-agent-files");
  assert!(shows(&mut desktop, "Found: AGENTS.md, CLAUDE.md, 2 skills folders"));
  desktop.click("settings-agent-files");
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the setting saved", |_| {
    ProjectDb::open(&repo)
      .and_then(|db| db.repository_agent_files())
      .is_ok_and(|setting| setting == RepositoryAgentFiles::Off)
  });
  desktop.settle("off shown", |tree| {
    tree
      .root()
      .is_some_and(|root| crate::agent_permissions::contains_text(root, "Off · agents load none of them"))
  });
  assert!(shows(&mut desktop, "Not loaded while this is off"));
}
