//! What a local MCP server's panel does besides editing: saving with the secret values typed, turning the server on or
//! off, asking for the program's file, and the one-line forms of its arguments and its tools that run without asking.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::mcp::{McpServerText, set_enabled_in, store_with_values},
  messages::Message,
  tasks::outside::Outside,
};

use super::editor::{EditorProps, Refusal, Save};
use crate::{
  screens::connections::mcp_form::fields::Fields,
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
};

/// Saves the server with the values typed for its secrets; the editor opens again on what was saved, and a refusal
/// leaves in `refused` the field it is about.
pub(super) fn save(
  ctx: &mut Ctx,
  props: &EditorProps,
  services: &Services,
  refused: &Signal<Refusal>,
) -> FutureAction<Save, (), Message> {
  let (services, revision, refused) = (services.clone(), props.revision.clone(), refused.clone());
  let (id, open) = (props.server.id(), props.props.project.open.clone());
  ctx.future_action(move |save: Save| {
    let (services, revision, open, refused) = (services.clone(), revision.clone(), open.clone(), refused.clone());
    async move {
      let spec = operations::connection_save(&open, save.draft.name.as_str());
      let vault = services.vault.clone();
      let stored = async move {
        background::run(move || {
          store_with_values(
            &save.root,
            vault.get(),
            (id, save.draft, &save.allowed),
            &save.values.values(),
          )
          .map_err(|error| {
            refused.set(Refusal::of(&error.field()));
            error.message()
          })
        })
        .await?
      };
      services.activity.track(spec, stored).await?;
      services.changes.record(open.project.id(), Outside::Connections);
      revision.update(|count| *count += 1);
      Ok(())
    }
  })
}

/// Turns the server on or off for every profile that chooses it, at once.
pub(super) fn switch(ctx: &mut Ctx, props: &EditorProps, services: &Services) -> FutureAction<bool, (), Message> {
  let (id, services) = (props.server.id(), services.clone());
  let (root, project) = (
    props.props.project.open.repo_root.clone(),
    props.props.project.open.project.id(),
  );
  ctx.future_action(move |enabled: bool| {
    let (root, services) = (root.clone(), services.clone());
    async move {
      background::run(move || {
        set_enabled_in(&root, id, enabled).map_err(|error| orchester_workspace::messages::store_error(&error))
      })
      .await??;
      services.changes.record(project, Outside::Connections);
      Ok(())
    }
  })
}

/// Asks for the program's file; the Program field takes the one chosen.
pub(super) fn choose(ctx: &mut Ctx, program: Signal<String>) -> FutureAction<(), Option<String>, Message> {
  let picker = PathPicker::of(ctx);
  let title = ctx.t("team_hub.fix.programTitle");
  let choose = ctx.future_action(move |(): ()| {
    let (picker, title) = (picker.clone(), title.clone());
    async move {
      Ok(
        picker
          .pick(PathKind::File, title)
          .await
          .map(|path| path.to_string_lossy().into_owned()),
      )
    }
  });
  ctx.watch(&choose.state(), move |state| {
    if let Some(Some(path)) = &state.data {
      program.set(path.clone());
    }
  });
  choose
}

/// The arguments and the tools that run without asking in one line each, which write the fields' lines as they are
/// typed.
pub(super) struct Lines {
  pub arguments: Signal<String>,
  pub tools: Signal<String>,
  /// Whether an argument has a space in it, which one line cannot say: the arguments are one a line then.
  pub one_line: bool,
}

impl Lines {
  pub(super) fn new(ctx: &mut Ctx, fields: &Fields, text: &McpServerText) -> Self {
    let arguments = ctx.signal(text.arguments.lines().collect::<Vec<_>>().join(" "));
    let tools = ctx.signal(text.exempt_tools.lines().collect::<Vec<_>>().join(", "));
    let one_line = !text.arguments.lines().any(|line| line.contains(char::is_whitespace));
    if one_line {
      let field = fields.arguments.clone();
      ctx.watch(&arguments, move |line| {
        field.set(line.split_whitespace().collect::<Vec<_>>().join("\n"));
      });
    }
    let field = fields.exempt_tools.clone();
    ctx.watch(&tools, move |line| {
      field.set(
        line
          .split(',')
          .map(str::trim)
          .filter(|tool| !tool.is_empty())
          .collect::<Vec<_>>()
          .join("\n"),
      );
    });
    Self {
      arguments,
      tools,
      one_line,
    }
  }
}
