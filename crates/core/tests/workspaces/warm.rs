use orchester_core::workspaces::{Cleaning, WarmChoice, WarmFolder, WarmPattern, WarmPatternError, cleaning};

fn pattern(text: &str) -> WarmPattern {
  WarmPattern::new(text).expect("a valid pattern")
}

#[test]
fn warm_pattern_reads_slashes_and_adds_the_closing_one() {
  assert_eq!(pattern("target").as_str(), "target/");
  assert_eq!(pattern(".\\packages\\*\\node_modules").as_str(), "packages/*/node_modules/");
  assert_eq!(pattern("packages/*/node_modules/").depth(), 3);
}

#[test]
fn warm_pattern_refuses_what_is_not_a_folder_of_the_repository() {
  assert_eq!(WarmPattern::new("  "), Err(WarmPatternError::Empty));
  assert!(matches!(WarmPattern::new("/etc/"), Err(WarmPatternError::OutsideRepository(_))));
  assert!(matches!(WarmPattern::new("C:/x/"), Err(WarmPatternError::OutsideRepository(_))));
  assert!(matches!(WarmPattern::new("a/../b/"), Err(WarmPatternError::BadPart(_))));
  assert!(matches!(WarmPattern::new("node_*/"), Err(WarmPatternError::Wildcard(_))));
  assert!(matches!(WarmPattern::new("packages/*/"), Err(WarmPatternError::Wildcard(_))));
  assert!(matches!(WarmPattern::new(".git/"), Err(WarmPatternError::GitFolder(_))));
}

#[test]
fn warm_pattern_matches_folders_and_what_lies_inside_them() {
  let nested = pattern("packages/*/node_modules/");
  assert!(nested.matches("packages/web/node_modules"));
  assert!(nested.matches("packages/web/node_modules/"));
  assert!(!nested.matches("packages/web/dist/"));
  assert!(!nested.matches("packages/node_modules/"));
  assert!(nested.holds("packages/web/node_modules/react/index.js"));
  assert!(!nested.holds("packages/web/src/index.js"));
}

#[test]
fn warm_clean_keeps_only_folders_set_to_keep() {
  let folders = [
    WarmFolder::new(pattern("target"), WarmChoice::Keep),
    WarmFolder::new(pattern("dist"), WarmChoice::Clean),
    WarmFolder::new(pattern("packages/*/node_modules"), WarmChoice::Keep),
  ];
  assert_eq!(cleaning(&folders, "target/"), Cleaning::Keep);
  assert_eq!(cleaning(&folders, "dist/"), Cleaning::Remove);
  assert_eq!(cleaning(&folders, ".tmp/"), Cleaning::Remove);
  assert_eq!(cleaning(&folders, "packages/web/node_modules/"), Cleaning::Keep);
  assert_eq!(cleaning(&folders, "packages/"), Cleaning::Descend);
  assert_eq!(cleaning(&folders, "packages/web/"), Cleaning::Descend);
  assert_eq!(cleaning(&folders, "packages/web/build.log"), Cleaning::Remove);
}
