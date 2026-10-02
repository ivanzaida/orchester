//! Permission rows of the step inspector (`PFnPS`, `Akfrj`, `iU0OB` and the note `m2XU9r` in
//! `Q21HOA`): repository access, commands and network of the step, which can only narrow its
//! profile's. Only values at or below the profile's are offered: read-only instead of editing, a
//! subset of the allowed commands (when the connection enforces a list), network off (when the
//! connection enforces it). Without a profile (`UjHDL`) one row says that permissions come from
//! the profile. A step that asks when a tool needs approval also lists, under that switch, the servers
//! of the profile that run some tools without asking (no frame draws these lines).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::invocation::Platform,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    StepNetwork, StepPermissions,
    capability::{PermissionOption, support},
  },
  resource::{StepKey, StepKind},
};

use super::{
  EditorHandle,
  step_inspector::{Fields, StepInspectorProps},
};
use crate::{
  agents::permissions::{escalations, file_command_hints, unlisted_commands},
  pipelines::{data::ProfileChoice, describe::repository_access, fields::AccessChoice},
  theme::ExtraTypography,
  ui::{
    command_chip::CommandChip,
    message_text,
    property::{ValueStyle, styled_property},
    switch::Switch,
  },
};

/// The permission rows of an agent step; none for a tool step.
pub fn rows(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields, handle: &EditorHandle) -> Vec<Element> {
  let StepKind::Agent {
    profile,
    repository,
    permissions: narrowing,
    ..
  } = &props.step.kind
  else {
    return Vec::new();
  };
  let Some(choice) = profile.and_then(|id| props.assignables.profile(id)) else {
    return vec![
      styled_property(
        &ctx.t("pipelines.inspector.permissions.title"),
        &ctx.t("pipelines.inspector.permissions.none"),
        ValueStyle::Mono,
      ),
      note(&ctx.t("pipelines.inspector.permissions.fromProfile")),
    ];
  };
  let effective = choice.permissions.narrowed(*repository, narrowing);
  let edit = StepEdit {
    handle: handle.clone(),
    key: props.step.key.clone(),
  };
  let mut rows = vec![repository_row(ctx, choice, &effective, fields)];
  rows.extend(commands_rows(ctx, choice, narrowing, &effective, &edit));
  rows.push(network_row(ctx, choice, narrowing, &effective, &edit));
  rows.push(tool_approvals_row(ctx, choice, narrowing, &edit));
  if choice.permissions.interactive_tool_approvals() || narrowing.interactive_tool_approvals {
    rows.extend(
      props
        .assignables
        .exempt_tools(choice)
        .into_iter()
        .map(|(server, count)| note(&exempt_text(ctx, &server, count))),
    );
  }
  rows.push(note(&narrowing_note(ctx, choice, narrowing)));
  rows
}

/// Changes the narrowing of the inspected step.
#[derive(Clone)]
struct StepEdit {
  handle: EditorHandle,
  key: StepKey,
}

impl StepEdit {
  fn apply(&self, change: impl FnOnce(&mut StepPermissions)) {
    self.handle.edit(|edit| {
      edit.update(&self.key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          change(permissions);
        }
      })
    });
  }
}

/// Read-only, or a choice of read-only and editing when the profile edits (`PFnPS`).
fn repository_row(ctx: &Ctx, choice: &ProfileChoice, effective: &AgentPermissions, fields: &Fields) -> Element {
  let label = ctx.t("pipelines.inspector.repositoryAccess");
  if choice.permissions.repository() == RepositoryAccess::ReadOnly {
    return styled_property(
      &label,
      &message_text(ctx, &repository_access(effective.repository())),
      ValueStyle::Mono,
    );
  }
  let options = [RepositoryAccess::ReadOnly, RepositoryAccess::Write]
    .into_iter()
    .map(|access| (AccessChoice(access), message_text(ctx, &repository_access(access))))
    .collect();
  super::step_fields::select_row(ctx, &label, "step-repository", &fields.access, options)
}

/// The step's commands (`Akfrj`), and when the profile allows a list the connection enforces,
/// each of them as an entry the step keeps or leaves out.
fn commands_rows(
  ctx: &Ctx,
  choice: &ProfileChoice,
  narrowing: &StepPermissions,
  effective: &AgentPermissions,
  edit: &StepEdit,
) -> Vec<Element> {
  let narrowed = narrowing.commands != StepCommands::AsProfile;
  let label = ctx.t(if narrowed {
    "pipelines.inspector.permissions.commandsNarrowed"
  } else {
    "pipelines.inspector.permissions.commands"
  });
  let mut rows = vec![styled_property(
    &label,
    &commands_value(ctx, effective),
    ValueStyle::Mono,
  )];
  rows.extend(escalations(ctx, choice.provider, effective.commands()).map(|text| note(&text)));
  let CommandPermission::Allowed(profile) = choice.permissions.commands() else {
    return rows;
  };
  let listable = choice
    .provider
    .is_some_and(|provider| support(provider, Platform::current(), PermissionOption::AllowedCommands).is_enforced());
  if !listable || profile.is_empty() {
    return rows;
  }
  let included = |entry: &CommandPrefix| match &narrowing.commands {
    StepCommands::AsProfile => true,
    StepCommands::Only(kept) => kept.entries().contains(entry),
  };
  let entries = profile.entries().iter().enumerate().map(|(index, entry)| {
    let chip = CommandChip::new(format!("step-command-{index}"), entry.as_str().into());
    let (edit, profile, toggled, keep) = (edit.clone(), profile.clone(), entry.clone(), !included(entry));
    let current = narrowing.commands.clone();
    let toggle = move || {
      let commands = toggled_commands(&profile, &current, &toggled, keep);
      edit.apply(|permissions| permissions.commands = commands.clone());
    };
    Element::from(if included(entry) {
      chip.on_remove(toggle)
    } else {
      chip.on_add(toggle)
    })
  });
  rows.push(
    Row::new()
      .id("step-commands")
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .overflow_visible()
      .with_children(entries)
      .into(),
  );
  if let CommandPermission::Allowed(kept) = effective.commands() {
    rows.extend(file_command_hints(ctx, kept).iter().map(|text| note(text)));
    rows.extend(unlisted_commands(ctx, choice.provider, kept).map(|text| note(&text)));
  }
  rows
}

/// The step's commands after adding (`keep`) or leaving out `toggled`: the profile's entries it
/// keeps, or the profile's own when it keeps all of them.
fn toggled_commands(
  profile: &AllowedCommands,
  current: &StepCommands,
  toggled: &CommandPrefix,
  keep: bool,
) -> StepCommands {
  let kept: Vec<CommandPrefix> = profile
    .entries()
    .iter()
    .filter(|entry| {
      let included = match current {
        StepCommands::AsProfile => true,
        StepCommands::Only(kept) => kept.entries().contains(entry),
      };
      if *entry == toggled { keep } else { included }
    })
    .cloned()
    .collect();
  if kept.len() == profile.entries().len() {
    return StepCommands::AsProfile;
  }
  StepCommands::Only(AllowedCommands::new(kept).expect("entries of a valid list stay unique"))
}

fn commands_value(ctx: &Ctx, effective: &AgentPermissions) -> Arc<str> {
  match effective.commands() {
    CommandPermission::AskBefore => ctx.t("agents.permissions.compact.commandsAsk"),
    CommandPermission::AnyInWorktree => ctx.t("agents.permissions.compact.commandsAny"),
    CommandPermission::Allowed(list) if list.is_empty() => ctx.t("agents.permissions.compact.commandsNone"),
    CommandPermission::Allowed(list) => {
      let entries: Vec<&str> = list.entries().iter().map(CommandPrefix::as_str).collect();
      entries.join(&ctx.t("common.listSeparator")).into()
    }
  }
}

/// Network (`iU0OB`): a switch when the profile has network on and the connection can turn it
/// off, else the value.
fn network_row(
  ctx: &Ctx,
  choice: &ProfileChoice,
  narrowing: &StepPermissions,
  effective: &AgentPermissions,
  edit: &StepEdit,
) -> Element {
  let label = ctx.t("agents.permissions.network");
  let can_turn_off = choice
    .provider
    .is_some_and(|provider| support(provider, Platform::current(), PermissionOption::NetworkOff).is_enforced());
  let value = match (effective.network(), can_turn_off) {
    (NetworkAccess::On, _) => ctx.t("agents.permissions.networkOn"),
    (NetworkAccess::Off, true) => ctx.t("agents.permissions.networkOff"),
    (NetworkAccess::Off, false) => ctx.t("pipelines.inspector.permissions.networkStaysOn"),
  };
  if choice.permissions.network() == NetworkAccess::Off || !can_turn_off {
    return styled_property(&label, &value, ValueStyle::Mono);
  }
  let (edit, network) = (edit.clone(), narrowing.network);
  let switch = Switch::new("step-network", value, effective.network() == NetworkAccess::On).render(move || {
    edit.apply(|permissions| {
      permissions.network = match network {
        StepNetwork::AsProfile => StepNetwork::Off,
        StepNetwork::Off => StepNetwork::AsProfile,
      };
    });
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&label).variant(TypographyStyle::Caption).flex(1.0))
    .child(switch)
    .into()
}

/// Opts this step into one-call decisions for tools that ask the host for approval.
fn tool_approvals_row(ctx: &Ctx, choice: &ProfileChoice, narrowing: &StepPermissions, edit: &StepEdit) -> Element {
  let label = ctx.t("pipelines.inspector.permissions.interactiveToolApprovals");
  let required = choice.permissions.interactive_tool_approvals();
  let enabled = required || narrowing.interactive_tool_approvals;
  let edit = edit.clone();
  let switch = Switch::new("step-tool-approvals", label.clone(), enabled).hide_label();
  let switch = if required || choice.provider.is_none() {
    switch.locked()
  } else {
    switch.render(move || {
      edit.apply(|permissions| permissions.interactive_tool_approvals = !enabled);
    })
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&label).variant(TypographyStyle::Caption).flex(1.0))
    .child(switch)
    .into()
}

/// "pencil: 5 tools run without asking".
fn exempt_text(ctx: &Ctx, server: &str, count: usize) -> Arc<str> {
  let key = if count == 1 {
    "pipelines.inspector.permissions.exemptTools.one"
  } else {
    "pipelines.inspector.permissions.exemptTools.other"
  };
  ctx.t_args(key, [("server", server), ("count", count.to_string().as_str())])
}

/// What the profile allows beyond the step (`m2XU9r`): "Reviewer v2 also allows pnpm lint.
/// Steps can only narrow profile permissions."
fn narrowing_note(ctx: &Ctx, choice: &ProfileChoice, narrowing: &StepPermissions) -> Arc<str> {
  let (CommandPermission::Allowed(profile), StepCommands::Only(kept)) =
    (choice.permissions.commands(), &narrowing.commands)
  else {
    return ctx.t("pipelines.inspector.permissions.onlyNarrow");
  };
  let left_out: Vec<&str> = profile
    .entries()
    .iter()
    .filter(|entry| !kept.entries().contains(entry))
    .map(CommandPrefix::as_str)
    .collect();
  if left_out.is_empty() {
    return ctx.t("pipelines.inspector.permissions.onlyNarrow");
  }
  ctx.t_args(
    "pipelines.inspector.permissions.alsoAllows",
    [
      ("profile", &*choice.name),
      ("version", choice.version.to_string().as_str()),
      ("commands", left_out.join(&ctx.t("common.listSeparator")).as_str()),
    ],
  )
}

/// The explanation under the rows (`m2XU9r`, `kaBUu`).
fn note(text: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(Text::new(text).variant(ExtraTypography::Hint.role()))
    .into()
}
