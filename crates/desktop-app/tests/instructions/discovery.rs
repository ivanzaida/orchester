//! The empty repository discovery card's layout and data, as drawn in B5A4Ux/T1w9U.
use std::fs;

use super::harness::Instructions;

#[test]
fn instructions_discovery_card_keeps_the_designed_gap_and_row_rhythm() {
  let mut screen = Instructions::new();
  fs::write(screen.root.join("AGENTS.md"), "Keep changes small.\n").expect("private instruction");
  fs::write(screen.root.join("CLAUDE.md"), "@AGENTS.md\n").expect("private reference");
  for folder in [".claude/skills", ".agents/skills"] {
    for name in ["build", "review"] {
      let path = screen.root.join(folder).join(name);
      fs::create_dir_all(&path).expect("private skill folder");
      fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Private fixture\n---\nFollow the plan.\n"),
      )
      .expect("private skill");
    }
  }
  screen.go();
  screen.until("repository discovery", |s| s.exists("instructions-repository-findings"));
  let text = screen.text("instructions-repository-findings");
  assert!(text.contains("Found in this repository · 6 items"));
  assert!(text.contains("Only points to AGENTS.md (@AGENTS.md) · nothing to import"));
  assert!(text.contains("2 skills · each becomes a draft skill in Skills"));
  assert!(text.contains("Creates drafts only: nothing is published and no repository file changes"));
  assert!(screen.exists("instructions-repository-path"));
  let mut bounds = |id: &str| {
    screen
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let action = bounds("instructions-empty-import");
  let card = bounds("instructions-repository-findings");
  let agents = bounds("instructions-finding-AGENTS.md");
  let claude = bounds("instructions-finding-CLAUDE.md");
  let skills = bounds("instructions-finding-.claude/skills");
  assert!(
    (card.y - action.y - action.height - 32.0).abs() < 1.0,
    "{action:?} {card:?}"
  );
  assert!((card.height - 228.0).abs() < 1.0, "{card:?}");
  assert!((claude.y - agents.y - 38.0).abs() < 1.0, "{agents:?} {claude:?}");
  assert!((skills.y - claude.y - 38.0).abs() < 1.0, "{claude:?} {skills:?}");
  assert!(screen.listed().entries.is_empty());
  screen.click("instructions-empty-import");
  screen.until("review", |s| s.exists("instruction-import-create"));
  assert!(screen.text("instruction-import-selection").contains("5 of 6 selected"));
  screen.click("instruction-import-cancel");
  screen.until("back to empty project", |s| {
    s.exists("instructions-repository-findings")
  });
  assert!(screen.listed().entries.is_empty());
}
