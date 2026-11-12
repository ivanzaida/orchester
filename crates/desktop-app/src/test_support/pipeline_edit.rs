//! A change put into the Pipelines hub's panel edit when the panel opens a pipeline, for steps the hub itself never
//! makes: steps with no order to draw (`Pipelines · v2` P16c `YHAyx`), which a stored version cannot hold. A test arms
//! it for a pipeline before the hub opens it ([`arm`]); the QA app started on the seed's `pipelines-unordered` arms it
//! from the marker the seed leaves beside the application database ([`write_marker`], [`arm_from_marker`]). The panel
//! takes it once, as if the person had made the change, so its notice, *Make it a line*, *Ready?* and the bar are
//! reached. Only the `test-support` feature builds it: the app's program has no such slot.

use std::{fs, io, path::Path};

use orchester_core::{
  id::ResourceId,
  resource::{PipelineEdit, StepKey},
};
use parking_lot::Mutex;

/// What is done to the edit of one pipeline.
pub type Change = Box<dyn FnOnce(&mut PipelineEdit) + Send>;

/// The marker beside the application database: the pipeline's id on its first line, then one `step dependency` pair
/// per line, each step made to wait for its dependency as well.
pub const MARKER: &str = "qa-app-pipeline-waits";

/// The armed changes, by pipeline. Each test arms its own pipeline, so tests that run side by side do not meet here.
static ARMED: Mutex<Vec<(ResourceId, Change)>> = parking_lot::const_mutex(Vec::new());

/// Makes `change` to the edit of `pipeline` when the hub's panel next opens it.
pub fn arm(pipeline: ResourceId, change: impl FnOnce(&mut PipelineEdit) + Send + 'static) {
  ARMED.lock().push((pipeline, Box::new(change)));
}

/// The change armed for `pipeline`, taken so it is made once.
pub(crate) fn take(pipeline: ResourceId) -> Option<Change> {
  let mut armed = ARMED.lock();
  let at = armed.iter().position(|(id, _)| *id == pipeline)?;
  Some(armed.remove(at).1)
}

/// Leaves the marker beside `database`: when the hub opens `pipeline`, each step of `waits` waits for its dependency
/// too.
pub fn write_marker(database: &Path, pipeline: ResourceId, waits: &[(StepKey, StepKey)]) -> io::Result<()> {
  let mut text = format!("{pipeline}\n");
  for (step, dependency) in waits {
    text.push_str(&format!("{step} {dependency}\n"));
  }
  fs::write(database.with_file_name(MARKER), text)
}

/// Arms the change the marker beside `database` names, if the seed left one; whether it did.
pub fn arm_from_marker(database: &Path) -> Result<bool, String> {
  let path = database.with_file_name(MARKER);
  let text = match fs::read_to_string(&path) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
    Err(error) => return Err(format!("{}: {error}", path.display())),
  };
  let mut lines = text.lines();
  let pipeline: ResourceId = lines
    .next()
    .ok_or("the marker names no pipeline")?
    .parse()
    .map_err(|error| format!("the marker's pipeline: {error}"))?;
  let waits = lines.map(pair).collect::<Result<Vec<_>, _>>()?;
  arm(pipeline, move |edit| {
    for (step, dependency) in &waits {
      edit.set_dependency(step, dependency, true);
    }
  });
  Ok(true)
}

/// A `step dependency` line of the marker.
fn pair(line: &str) -> Result<(StepKey, StepKey), String> {
  let (step, dependency) = line
    .split_once(' ')
    .ok_or_else(|| format!("not a step and a dependency: {line}"))?;
  let key = |text: &str| StepKey::new(text).map_err(|error| format!("{text}: {error}"));
  Ok((key(step)?, key(dependency)?))
}
