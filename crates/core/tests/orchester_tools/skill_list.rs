//! `list_skills` keeps its answer small: a long description ends with `…`, reference files that do not fit are
//! counted, skills past the answer's bounds are named alone and then counted, and the answer says what it cut and
//! that `read_skill` reads a skill whole.

use std::{fs, path::Path};

use orchester_core::{
  orchester_tools::{SkillCall, StepSkills},
  resource::SkillFilePath,
};

/// The list's bound on its entries, and room for the notes after them.
const LIST_CHARS: usize = 12_000;
const NOTES_CHARS: usize = 1_000;

/// Offers Orchester's skill `name` placed in the folder `folder`, with `description` and the reference files `files`.
fn place(skills: &mut StepSkills, folder: &Path, (name, description): (&str, &str), files: &[SkillFilePath]) {
  let document = folder.join("SKILL.md");
  fs::create_dir_all(folder).expect("a folder");
  fs::write(&document, format!("---\nname: {name}\n---\n")).expect("a SKILL.md");
  skills
    .add_placed((name, description), &document, files)
    .expect("placed");
}

fn listed(skills: &StepSkills) -> String {
  let listed = skills.run(&SkillCall::List);
  assert!(!listed.is_error());
  listed.text().to_owned()
}

/// The count in the line of `text` that starts with `start`, if there is one.
fn counted(text: &str, start: &str) -> Option<usize> {
  let line = text.lines().find_map(|line| line.strip_prefix(start))?;
  line.split(' ').next()?.parse().ok()
}

#[test]
fn orchester_tools_list_skills_cuts_a_long_description_and_counts_the_files_past_its_bounds() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let mut skills = StepSkills::default();
  let long = "Deploys the app. ".repeat(40);
  let files: Vec<SkillFilePath> = (0..100)
    .map(|index| SkillFilePath::new(&format!("references/step-{index:03}.md")).expect("a path"))
    .collect();
  place(&mut skills, &root.path().join("deploy"), ("deploy", &long), &files);
  place(&mut skills, &root.path().join("lint"), ("lint", "Lints."), &[]);
  let text = listed(&skills);

  let entry = text
    .lines()
    .find(|line| line.starts_with("- deploy (orchester): "))
    .expect("deploy is listed");
  let description = entry.trim_start_matches("- deploy (orchester): ");
  assert_eq!(description.chars().count(), 300, "{entry}");
  assert!(description.starts_with("Deploys the app. Deploys the app."), "{entry}");
  assert!(description.ends_with('…'), "{entry}");

  let files_line = text
    .lines()
    .find(|line| line.starts_with("  reference files: references/step-000.md"))
    .expect("deploy's files");
  let shown = files_line.matches("references/step-").count();
  assert!(shown < files.len(), "{files_line}");
  assert!(
    files_line.ends_with(&format!(", [{} more not listed]", files.len() - shown)),
    "{files_line}"
  );

  assert!(
    text.contains("- lint (orchester): Lints.\n  reference files: none\n"),
    "{text}"
  );
  assert_eq!(
    counted(&text, "Cut short to keep this answer small: "),
    Some(1),
    "{text}"
  );
  assert!(
    text.contains("read_skill with a skill's name reads it whole."),
    "{text}"
  );
  assert!(!text.contains("Named only"), "{text}");
  assert!(!text.contains("Not listed"), "{text}");
}

#[test]
fn orchester_tools_list_skills_names_and_then_counts_the_skills_past_its_bounds() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let mut skills = StepSkills::default();
  let offered = 200;
  let description = "Checks one part of the app. ".repeat(10);
  for index in 0..offered {
    let name = format!("skill-{index:03}-{}", "x".repeat(150));
    // A short folder: a path with the long name could pass what some systems allow.
    place(
      &mut skills,
      &root.path().join(index.to_string()),
      (&name, &description),
      &[],
    );
  }
  let text = listed(&skills);

  assert!(
    text.chars().count() <= LIST_CHARS + NOTES_CHARS,
    "{} characters",
    text.chars().count()
  );
  let whole = text.lines().filter(|line| line.contains(" (orchester): ")).count();
  let named = text
    .lines()
    .filter(|line| line.starts_with("- skill-") && line.ends_with(" (orchester)"))
    .count();
  let unlisted = counted(&text, "Not listed, to keep this answer small: ").expect("the skills not listed are counted");
  assert!(whole > 0 && unlisted > 0, "{text}");
  assert_eq!(whole + named + unlisted, offered, "{text}");
  if named > 0 {
    assert_eq!(
      counted(&text, "Named only, to keep this answer small: "),
      Some(named),
      "{text}"
    );
  }
  assert!(
    text.contains(&format!(
      "Not listed, to keep this answer small: {unlisted} more skills offered."
    )),
    "{text}"
  );
}

#[test]
fn orchester_tools_list_skills_adds_no_note_when_nothing_is_cut() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let mut skills = StepSkills::default();
  let listed_file = [SkillFilePath::new("references/checks.md").expect("a path")];
  place(
    &mut skills,
    &root.path().join("run-checks"),
    ("run-checks", "Runs the checks."),
    &listed_file,
  );
  assert_eq!(
    listed(&skills),
    "Read a skill with read_skill and its name; a reference file with its file too.\n- run-checks (orchester): Runs \
     the checks.\n  reference files: references/checks.md\n"
  );
}
