//! Skills (ORC-15): the resource and its publishing rules, the `SKILL.md` an agent reads, and what
//! Import from repository makes of a repository's agent files.

use std::sync::Arc;

use orchester_core::{
  agent_files::{
    frontmatter,
    import::{Candidate, DraftContent, Found, FoundFile, ItemKind, RepositoryDraftError, Unimportable, candidates},
  },
  blob::ContentHash,
  name::Name,
  resource::{InstructionScope, PublishError, ResourceBody, Skill, SkillError, SkillFile, SkillFilePath, Slug},
};

fn hash(digit: char) -> ContentHash {
  ContentHash::new(&digit.to_string().repeat(64)).expect("valid hash")
}

fn file(path: &str, digit: char) -> SkillFile {
  SkillFile {
    path: SkillFilePath::new(path).expect("valid path"),
    content: hash(digit),
    size: 10,
  }
}

#[test]
fn skill_file_paths_stay_inside_the_skill_folder_on_both_platforms() {
  for valid in ["references/checks.md", "scripts/run.ps1", "a.txt", "deep/er/file"] {
    assert!(SkillFilePath::new(valid).is_ok(), "{valid}");
  }
  for invalid in [
    "",
    "SKILL.md",
    "/etc/passwd",
    "../up.md",
    "a/../b",
    "a//b",
    "C:/x",
    "a\\b",
    "dot./x",
    "q?.md",
    "./a",
  ] {
    assert!(
      matches!(SkillFilePath::new(invalid), Err(SkillError::InvalidPath(_))),
      "{invalid}"
    );
  }
  assert!(ContentHash::new("abc").is_err());
  assert!(ContentHash::new(&"A".repeat(64)).is_err());
}

#[test]
fn skill_keeps_its_files_in_path_order_and_refuses_two_at_one_path() {
  let skill = Skill::new(
    "d".into(),
    "b".into(),
    vec![file("z.md", 'a'), file("references/a.md", 'b')],
    InstructionScope::Project,
  )
  .expect("valid");
  let paths: Vec<&str> = skill.files().iter().map(|file| file.path.as_str()).collect();
  assert_eq!(paths, ["references/a.md", "z.md"]);
  let twice = Skill::new(
    "d".into(),
    "b".into(),
    vec![file("a.md", 'a'), file("a.md", 'b')],
    InstructionScope::Project,
  );
  assert_eq!(twice, Err(SkillError::DuplicateFile("a.md".into())));
}

#[test]
fn a_skill_is_published_with_a_description_and_a_body() {
  let skill = |description: &str, body: &str| {
    Skill::new(description.into(), body.into(), Vec::new(), InstructionScope::Project).expect("valid draft")
  };
  assert_eq!(
    skill(" ", "Body").check_publishable(),
    Err(PublishError::BlankSkillDescription)
  );
  assert_eq!(
    skill("What it does", "\n").check_publishable(),
    Err(PublishError::BlankSkillBody)
  );
  assert_eq!(skill("What it does", "Body").check_publishable(), Ok(()));
}

#[test]
fn stored_skill_bodies_are_checked_when_read() {
  let skill = Skill::new(
    "d".into(),
    "b".into(),
    vec![file("a.md", 'a')],
    InstructionScope::Project,
  )
  .expect("valid");
  let mut stored = serde_json::to_value(&skill).expect("serializes");
  assert_eq!(serde_json::from_value::<Skill>(stored.clone()).expect("reads"), skill);
  stored["files"][0]["path"] = "../escape.md".into();
  assert!(serde_json::from_value::<Skill>(stored).is_err());
}

#[test]
fn the_skill_document_quotes_its_name_and_description() {
  let skill = Skill::new(
    "Runs checks: \"all\" of them\nbefore a PR.".into(),
    "\n# Run checks\n\nRun them.\n".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("valid");
  let document = skill.document(&Slug::new("run-checks").expect("valid"));
  assert_eq!(
    document,
    "---\nname: \"run-checks\"\ndescription: \"Runs checks: \\\"all\\\" of them\\nbefore a PR.\"\n---\n\n# Run \
     checks\n\nRun them.\n"
  );
  let parsed = frontmatter::parse(&document);
  assert_eq!(parsed.name.as_deref(), Some("run-checks"));
  assert_eq!(
    parsed.description.as_deref(),
    Some("Runs checks: \"all\" of them\nbefore a PR.")
  );
  assert_eq!(parsed.body, "# Run checks\n\nRun them.");
}

#[test]
fn front_matter_reads_the_scalar_forms_skill_files_use() {
  let plain =
    frontmatter::parse("---\nname: run-checks\ndescription: Runs the checks\n  before a PR.\nother: x\n---\nBody\n");
  assert_eq!(plain.name.as_deref(), Some("run-checks"));
  assert_eq!(plain.description.as_deref(), Some("Runs the checks before a PR."));
  assert_eq!(plain.body, "Body");

  let quoted = frontmatter::parse("\u{feff}---\r\nname: 'it''s'\r\ndescription: \"Tab\\there\"\r\n---\r\nBody");
  assert_eq!(quoted.name.as_deref(), Some("it's"));
  assert_eq!(quoted.description.as_deref(), Some("Tab\there"));

  let literal = frontmatter::parse("---\nname: x\ndescription: |\n  First line\n  Second line\n---\n");
  assert_eq!(literal.description.as_deref(), Some("First line\nSecond line"));
  let folded = frontmatter::parse("---\ndescription: >-\n  First line\n  second line\nname: y\n---\n");
  assert_eq!(folded.description.as_deref(), Some("First line second line"));
  assert_eq!(folded.name.as_deref(), Some("y"));

  let none = frontmatter::parse("# Just Markdown\n\n---\n");
  assert_eq!((none.name, none.description), (None, None));
  assert_eq!(none.body, "# Just Markdown\n\n---");
  let unclosed = frontmatter::parse("---\nname: x\n");
  assert_eq!(unclosed.name, None);
}

fn found_file(path: &str, content: &[u8]) -> FoundFile {
  FoundFile {
    path: SkillFilePath::new(path).expect("valid"),
    content: Arc::from(content),
  }
}

fn kontur() -> Vec<Found> {
  vec![
    Found::Skill {
      folder: ".agents/skills/old-deploy".into(),
      markdown: None,
      files: Vec::new(),
      unsupported: Vec::new(),
    },
    Found::Instructions {
      path: "CLAUDE.md".into(),
      content: b"@AGENTS.md\n".to_vec(),
    },
    Found::Skill {
      folder: ".claude/skills/run-checks".into(),
      markdown: Some(b"---\nname: run-checks\ndescription: Runs the checks.\n---\n\nRun {{them}}.\n".to_vec()),
      files: vec![found_file("references/checks.md", b"cargo test")],
      unsupported: Vec::new(),
    },
    Found::Instructions {
      path: "AGENTS.md".into(),
      content: b"# Kontur\n\nUse {{task.id}} literally.\n".to_vec(),
    },
    Found::Skill {
      folder: ".agents/skills/github-oauth".into(),
      markdown: Some(b"No front matter.".to_vec()),
      files: Vec::new(),
      unsupported: Vec::new(),
    },
  ]
}

#[test]
fn import_lists_instruction_files_then_skill_folders_with_what_each_becomes() {
  let items = candidates(kontur());
  let listed: Vec<(ItemKind, &str)> = items.iter().map(|item| (item.kind, item.source.as_str())).collect();
  assert_eq!(
    listed,
    [
      (ItemKind::Instruction, "AGENTS.md"),
      (ItemKind::Instruction, "CLAUDE.md"),
      (ItemKind::Skill, ".claude/skills/run-checks"),
      (ItemKind::Skill, ".agents/skills/github-oauth"),
      (ItemKind::Skill, ".agents/skills/old-deploy"),
    ]
  );
  assert_eq!(
    items[1].outcome,
    Err(Unimportable::OnlyImports {
      targets: vec!["AGENTS.md".into()]
    })
  );
  assert_eq!(items[4].outcome, Err(Unimportable::NoSkillFile));
  let Ok(Candidate::Instruction { name, text }) = &items[0].outcome else {
    panic!("AGENTS.md becomes an instruction");
  };
  assert_eq!(name.as_str(), "AGENTS.md");
  // Nothing in a repository file reads as a variable: it goes to agents as written.
  assert!(text.variables().is_empty());
  assert_eq!(
    text.render(|_| None).expect("no variables"),
    "# Kontur\n\nUse {{task.id}} literally.\n"
  );
  let Ok(Candidate::Skill {
    name,
    description,
    body,
    files,
  }) = &items[2].outcome
  else {
    panic!("a skill folder becomes a skill");
  };
  assert_eq!(
    (name.as_str(), description.as_str(), body.as_str()),
    ("run-checks", "Runs the checks.", "Run {{them}}.")
  );
  assert_eq!(files.len(), 1);
  let Ok(Candidate::Skill { name, description, .. }) = &items[3].outcome else {
    panic!("a skill without front matter is named after its folder");
  };
  assert_eq!((name.as_str(), description.as_str()), ("github-oauth", ""));
}

#[test]
fn import_leaves_out_empty_and_binary_instruction_files() {
  let items = candidates(vec![
    Found::Instructions {
      path: "AGENTS.md".into(),
      content: b"  \n\n".to_vec(),
    },
    Found::Instructions {
      path: "CLAUDE.md".into(),
      content: vec![0xff, 0xfe, 0x00],
    },
  ]);
  assert_eq!(items[0].outcome, Err(Unimportable::Empty));
  assert_eq!(items[1].outcome, Err(Unimportable::NotText));
  // A CLAUDE.md with text of its own besides an import is imported as it is.
  let mixed = candidates(vec![Found::Instructions {
    path: "CLAUDE.md".into(),
    content: b"@AGENTS.md\n\nAlso run the checks.".to_vec(),
  }]);
  assert!(matches!(mixed[0].outcome, Ok(Candidate::Instruction { .. })));
  // A skill with a file it could not hold is not imported without it.
  let lossy = candidates(vec![Found::Skill {
    folder: ".claude/skills/x".into(),
    markdown: Some(b"Body".to_vec()),
    files: Vec::new(),
    unsupported: vec!["bad:name.md".into()],
  }]);
  assert_eq!(
    lossy[0].outcome,
    Err(Unimportable::UnsupportedFiles {
      paths: vec!["bad:name.md".into()]
    })
  );
}

#[test]
fn drafts_get_free_slugs_the_project_scope_and_their_stored_files() {
  let items = candidates(kontur());
  let mut instruction_slugs = vec![Slug::new("agents-md").expect("valid")];
  let Ok(agents) = &items[0].outcome else {
    panic!("importable")
  };
  let draft = agents
    .draft(&items[0].source, &mut instruction_slugs, Vec::new())
    .expect("drafts");
  assert_eq!(draft.slug.as_str(), "agents-md-2");
  assert_eq!(draft.source, "AGENTS.md");
  assert!(matches!(draft.content, DraftContent::Instruction(ref body) if *body.scope() == InstructionScope::Project));

  let Ok(run_checks) = &items[2].outcome else {
    panic!("importable")
  };
  let mut skill_slugs = Vec::new();
  assert_eq!(
    run_checks.draft(".claude/skills/run-checks", &mut skill_slugs, Vec::new()),
    Err(RepositoryDraftError::FilesMismatch)
  );
  let draft = run_checks
    .draft(
      ".claude/skills/run-checks",
      &mut skill_slugs,
      vec![file("references/checks.md", 'c')],
    )
    .expect("drafts");
  let DraftContent::Skill(skill) = &draft.content else {
    panic!("a skill")
  };
  assert_eq!(draft.name, Name::new("run-checks").expect("valid"));
  assert_eq!(skill.scope(), &InstructionScope::Project);
  assert_eq!(skill.files()[0].content, hash('c'));
  // A second skill of the same name gets the next free slug.
  let again = run_checks
    .draft(
      ".agents/skills/run-checks",
      &mut skill_slugs,
      vec![file("references/checks.md", 'c')],
    )
    .expect("drafts");
  assert_eq!(again.slug.as_str(), "run-checks-2");
}
