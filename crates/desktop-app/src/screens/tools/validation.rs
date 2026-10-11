//! Presentation of the workspace's command validation; disk checks stay off the event loop.
use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::resource::Tool;
use orchester_workspace::{
  background,
  tools::{
    data::ToolEntry,
    editor::{self, Edit},
    form::{CommandErrors, tool_name},
  },
};

use super::{editor::EditorProps, fields::Fields};
#[derive(Clone, Debug, PartialEq, Eq)]
struct Prepared {
  given: Option<Tool>,
  fixed: Option<Tool>,
}
impl DevtoolsInspectable for Prepared {}

pub(super) fn checked(
  ctx: &mut Ctx,
  fields: &Fields,
  baseline: Option<&ToolEntry>,
  p: &EditorProps,
) -> (Result<Edit, CommandErrors>, bool) {
  let name = match baseline {
    Some(e) => tool_name(&e.name),
    None => editor::new_name(&p.data.listed, &fields.name.get()),
  };
  let parsed = fields.text().tool(baseline.and_then(|e| e.tool.access().cloned()));
  let given = parsed.as_ref().ok().cloned();
  let root = p.project.open.repo_root.clone();
  let reading = ctx
    .future((root, given.clone(), p.read_revision), |(root, given, _)| async move {
      let source = given.clone();
      let fixed = match given {
        Some(tool) => Some(background::run(move || editor::prepare_in(&root, &tool)).await??),
        None => None,
      };
      Ok::<_, orchester_workspace::messages::Message>(Prepared { given: source, fixed })
    })
    .state()
    .get();
  let mut errors = parsed.err().map(|e| *e).unwrap_or_default();
  errors.name = name.as_ref().err().cloned();
  let current = reading.data.as_ref().filter(|d| d.given == given);
  let pending = given.is_some() && (reading.is_pending() || (reading.error.is_none() && current.is_none()));
  if !reading.is_pending()
    && let Some(error) = reading.error.as_ref()
  {
    errors.program = Some(error.clone());
  }
  let checked = match (name, current.and_then(|d| d.fixed.clone())) {
    (Ok(name), Some(tool)) if errors == CommandErrors::default() => Ok(Edit::Save {
      name,
      tool: Box::new(tool),
      before: baseline.cloned(),
    }),
    _ => Err(errors),
  };
  (checked, pending)
}
