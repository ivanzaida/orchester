//! The code index folder a step's prompt names, as the fake CLIs read it to search the index (ORC-90).

/// The folder of the prompt's *Codebase index* part: "An index of this repository at the run's base revision <rev>
/// is in <folder>: map.md …".
pub fn index_folder(prompt: &str) -> Option<String> {
  let line = prompt
    .lines()
    .find(|line| line.starts_with("An index of this repository"))?;
  let (_, rest) = line.split_once(" is in ")?;
  let (folder, _) = rest.split_once(": map.md")?;
  Some(folder.to_owned())
}
