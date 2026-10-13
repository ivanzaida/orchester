//! The system's Recycle Bin / Trash, by hand only: it moves a folder into the person's own bin. No other test reaches
//! it; the workspace's tests and QA copies use a trash folder of their own (`orchester_workspace::accounts::trash`).

#[test]
#[ignore = "moves a folder into the person's real Recycle Bin / Trash; run by hand: cargo test -p orchester-exec --test trash -- --ignored"]
fn trash_moves_a_folder_to_the_systems_bin() {
  let folder = tempfile::tempdir().expect("a temporary folder").keep();
  std::fs::write(folder.join("note.txt"), "Orchester's trash check").expect("a file");
  orchester_exec::trash::move_to_trash(&folder).expect("moved");
  assert!(!folder.exists(), "the folder left its place for the bin");
}
