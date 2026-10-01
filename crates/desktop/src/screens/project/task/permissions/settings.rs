//! The three settings of the run permissions popover in compact form (`G2l6MT`, `fxPh3`,
//! `q4Nelz` in `NNRH8`): repository access and commands as radio options, the allowed commands as
//! entries that remove themselves with an add button, and network as a switch. An option the
//! step's connection cannot enforce is faded with the reason and cannot be chosen, as in the
//! profile editor (04.B); commands the run adds to the profile's are outlined and named.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, Support, support},
  },
};

use super::RunPermissionsProps;
use crate::{
  agents::permissions::{
    escalations, file_command_hints, option_title, unavailable, unlisted_commands, with_command, without_command,
  },
  messages::Message,
  theme::{ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY},
  ui::{
    command_chip::CommandChip,
    field::{field_error, text_input},
    form::form,
    icon::{Icon, IconSize, icon},
    message_text,
    option_card::unavailable_reason,
    pressable::{Ring, keyboard_stop},
    switch::Switch,
  },
};

/// Height of a command entry, the add button and the command field (`bpmzN`, `WMKNn`).
const COMMAND_HEIGHT: f32 = 26.0;
/// Width of the command field that the add button opens.
const COMMAND_INPUT_WIDTH: f32 = 140.0;

/// The permissions as changed so far in the popover; `None` until it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Draft(pub(super) Option<AgentPermissions>);

impl lurq::app::component::DevtoolsInspectable for Draft {}

/// What the popover's settings edit, kept while it is open.
#[derive(Clone)]
pub(super) struct Edit {
  pub(super) draft: Signal<Draft>,
  /// *Also save to the profile* is checked.
  pub(super) save: Signal<bool>,
  pub(super) new_command: Signal<String>,
  pub(super) command_error: Signal<Option<Message>>,
  /// The command field is shown.
  pub(super) adding: Signal<bool>,
  pub(super) command_input: ElementRef,
}

impl Edit {
  /// Starts editing `base` afresh.
  pub(super) fn open(&self, base: &AgentPermissions) {
    self.draft.set(Draft(Some(base.clone())));
    self.save.set(false);
    self.new_command.set(String::new());
    self.command_error.set(None);
    self.adding.set(false);
  }

  fn change(&self, change: impl FnOnce(&mut AgentPermissions)) {
    self.draft.update(|draft| {
      if let Some(draft) = &mut draft.0 {
        change(draft);
      }
    });
  }
}

/// Repository access, commands and network for `draft`.
pub(super) fn settings(ctx: &Ctx, edit: &Edit, props: &RunPermissionsProps, draft: &AgentPermissions) -> Vec<Element> {
  let provider = props.step.provider;
  vec![
    repository(ctx, edit, provider, draft),
    commands(ctx, edit, props, draft),
    network(ctx, edit, provider, draft),
    tool_approvals(ctx, edit, props, draft),
  ]
}

/// A setting's label over its options (`G2l6MT`).
fn group(ctx: &Ctx, label: &str, content: Vec<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .overflow_visible()
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Caption))
    .with_children(content)
    .into()
}

/// How `provider` supports `option` here; without a connection nothing can be enforced.
fn supported(provider: Option<AgentProvider>, option: PermissionOption) -> Option<Support> {
  provider.map(|provider| support(provider, Platform::current(), option))
}

/// Why `option` cannot be chosen, if it cannot.
fn reason(ctx: &Ctx, provider: Option<AgentProvider>, option: PermissionOption) -> Option<Option<Arc<str>>> {
  match (provider, supported(provider, option)) {
    (Some(_), Some(Support::Enforced(_))) => None,
    (Some(provider), Some(Support::Unavailable(reason))) => Some(unavailable(ctx, reason, provider)),
    _ => Some(None),
  }
}

/// A compact radio option (`ihdDb`): chosen by a press, or faded when it cannot be chosen.
fn radio(
  ctx: &Ctx,
  (id, option): (&'static str, PermissionOption),
  selected: bool,
  available: bool,
  choose: impl Fn() + Send + Sync + 'static,
) -> Element {
  let (glyph, color) = if selected {
    (Icon::CircleDot, PaletteColor::TextPrimary)
  } else {
    (Icon::Circle, PaletteColor::TextSecondary)
  };
  let row = Button::empty()
    .button()
    .id(id)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .describe("selected", if selected { "true" } else { "false" })
    .child(icon(glyph, IconSize::Event, color.clone()))
    .child(
      Text::new(&ctx.t(option_title(option)))
        .variant(TypographyStyle::FieldLabel)
        .color(color),
    );
  if !available {
    return row
      .focusable(false)
      .cursor(CursorIcon::NotAllowed)
      .opacity(UNAVAILABLE_OPTION_OPACITY)
      .describe("state", "unavailable")
      .into();
  }
  keyboard_stop(row, Ring::Outer)
    .cursor(CursorIcon::Pointer)
    .describe("state", "available")
    .on_click(move |_| choose())
    .into()
}

fn options(options: Vec<Element>) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Step.role())
    .align_items(Alignment::Center)
    .overflow_visible()
    .with_children(options)
    .into()
}

/// The reasons of the options that cannot be chosen, under the options.
fn reasons(reasons: impl IntoIterator<Item = Option<Option<Arc<str>>>>) -> impl Iterator<Item = Element> {
  reasons
    .into_iter()
    .flatten()
    .flatten()
    .map(|reason| unavailable_reason(&reason))
}

fn repository(ctx: &Ctx, edit: &Edit, provider: Option<AgentProvider>, draft: &AgentPermissions) -> Element {
  let current = draft.repository();
  let choice = |id, option, access: RepositoryAccess| {
    let edit = edit.clone();
    let why = reason(ctx, provider, option);
    let element = radio(ctx, (id, option), current == access, why.is_none(), move || {
      edit.change(|draft| draft.set_repository(access));
    });
    (element, why)
  };
  let (read_only, read_only_why) = choice(
    "task-permissions-read-only",
    PermissionOption::ReadOnly,
    RepositoryAccess::ReadOnly,
  );
  let (edit_files, edit_why) = choice(
    "task-permissions-edit",
    PermissionOption::EditWorktree,
    RepositoryAccess::Write,
  );
  let mut content = vec![options(vec![read_only, edit_files])];
  content.extend(reasons([read_only_why, edit_why]));
  group(ctx, "agents.permissions.repository", content)
}

fn commands(ctx: &Ctx, edit: &Edit, props: &RunPermissionsProps, draft: &AgentPermissions) -> Element {
  let provider = props.step.provider;
  let listed = match draft.commands() {
    CommandPermission::Allowed(list) => Some(list.clone()),
    CommandPermission::AskBefore | CommandPermission::AnyInWorktree => None,
  };
  let kept = listed.clone().unwrap_or_default();
  let allowed_why = reason(ctx, provider, PermissionOption::AllowedCommands);
  let any_why = reason(ctx, provider, PermissionOption::AnyCommand);
  let ask_why = reason(ctx, provider, PermissionOption::AskBeforeCommands);
  let ask_edit = edit.clone();
  let (allowed_edit, any_edit) = (edit.clone(), edit.clone());
  let mut content = vec![
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .with_children(vec![
        radio(
          ctx,
          ("task-permissions-ask", PermissionOption::AskBeforeCommands),
          *draft.commands() == CommandPermission::AskBefore,
          ask_why.is_none(),
          move || ask_edit.change(|draft| draft.set_commands(CommandPermission::AskBefore)),
        ),
        radio(
          ctx,
          ("task-permissions-allowed", PermissionOption::AllowedCommands),
          listed.is_some(),
          allowed_why.is_none(),
          move || allowed_edit.change(|draft| draft.set_commands(CommandPermission::Allowed(kept.clone()))),
        ),
        radio(
          ctx,
          ("task-permissions-any", PermissionOption::AnyCommand),
          *draft.commands() == CommandPermission::AnyInWorktree,
          any_why.is_none(),
          move || any_edit.change(|draft| draft.set_commands(CommandPermission::AnyInWorktree)),
        ),
      ])
      .into(),
  ];
  content.extend(reasons([ask_why, allowed_why, any_why]));
  content.extend(escalations(ctx, provider, draft.commands()).map(|note| hint(&note)));
  if let Some(list) = listed {
    content.push(command_entries(ctx, edit, &list, props));
    content.extend(changed_note(ctx, &list, props));
    if let Some(error) = edit.command_error.get() {
      content.push(field_error(&message_text(ctx, &error)));
    }
    content.extend(file_command_hints(ctx, &list).iter().map(|note| hint(note)));
    content.extend(unlisted_commands(ctx, provider, &list).map(|note| hint(&note)));
  }
  group(ctx, "agents.permissions.commands", content)
}

fn tool_approvals(ctx: &Ctx, edit: &Edit, props: &RunPermissionsProps, draft: &AgentPermissions) -> Element {
  let provider = props.step.provider;
  let required = props.permissions.tool_approvals_required;
  let enabled = required || draft.interactive_tool_approvals();
  let switch = Switch::new(
    "task-permissions-tool-approvals",
    ctx.t("agents.permissions.toolApprovals"),
    enabled,
  );
  let control = if !required && (provider.is_some() || enabled) {
    let edit = edit.clone();
    switch.render(move || edit.change(|draft| draft.set_interactive_tool_approvals(!enabled)))
  } else {
    switch.locked()
  };
  let mut controls = vec![control];
  let transport = match provider {
    Some(AgentProvider::CodexCli) => Some("agents.permissions.transport"),
    Some(AgentProvider::ClaudeCodeCli) => Some("agents.permissions.claudeTransport"),
    None => None,
  };
  if let Some(transport) = transport.filter(|_| enabled || *draft.commands() == CommandPermission::AskBefore) {
    controls.push(hint(&ctx.t(transport)));
  }
  group(ctx, "agents.permissions.toolApprovalsHeading", controls)
}

/// The profile version's allowed commands; none when it allows no list.
fn profile_commands(props: &RunPermissionsProps) -> AllowedCommands {
  match props.permissions.profile_permissions.commands() {
    CommandPermission::Allowed(list) => list.clone(),
    CommandPermission::AskBefore | CommandPermission::AnyInWorktree => AllowedCommands::default(),
  }
}

/// The entries (`V6Cu3L`): each removes itself; one the profile does not list is outlined. Then the
/// add button (`WMKNn`), or the field it opened, where Enter adds the command.
fn command_entries(ctx: &Ctx, edit: &Edit, list: &AllowedCommands, props: &RunPermissionsProps) -> Element {
  let profile = profile_commands(props);
  let entries = list.entries().iter().enumerate().map(|(index, entry)| {
    let (edit, removed) = (edit.clone(), entry.clone());
    Element::from(
      CommandChip::new(format!("task-permissions-command-{index}"), entry.as_str().into())
        .changed(!profile.allows(entry))
        .on_remove(move || {
          edit.change(|draft| {
            if let CommandPermission::Allowed(list) = draft.commands() {
              let rest = without_command(list, &removed);
              draft.set_commands(CommandPermission::Allowed(rest));
            }
          });
        }),
    )
  });
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .overflow_visible()
    .with_children(entries);
  if !edit.adding.get() {
    let open = edit.adding.clone();
    return row.child(add_button(move || open.set(true))).into();
  }
  let add = add_command(edit);
  let input = text_input(
    ctx,
    "task-permissions-new-command",
    edit.new_command.clone(),
    edit.command_error.get().is_some(),
    ExtraTypography::MonoValue.role(),
  )
  .size(Dimension::Px(COMMAND_INPUT_WIDTH), Dimension::Px(COMMAND_HEIGHT))
  .placeholder(&ctx.t("agents.permissions.commandPlaceholder"))
  .ref_element(edit.command_input.clone());
  ctx.focus(&edit.command_input);
  // Enter in the command field adds the command rather than applying the change.
  form(Some(add.clone()), row.child(input).child(add_button(move || add())))
}

/// The icon button that adds a command (`WMKNn`, tooltip *Add command*).
fn add_button(add: impl Fn() + Send + Sync + 'static) -> Element {
  keyboard_stop(Button::empty().button(), Ring::Outer)
    .id("task-permissions-add-command")
    .width(Dimension::Px(COMMAND_HEIGHT))
    .height(Dimension::Px(COMMAND_HEIGHT))
    .align_items(Alignment::Center)
    .justify(lurq::layout::layout_kind::Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| add())
    .child(icon(Icon::Plus, IconSize::Marker, PaletteColor::TextPrimary))
    .into()
}

/// Adds the typed command to the draft, or says why it cannot be added.
fn add_command(edit: &Edit) -> Arc<dyn Fn() + Send + Sync> {
  let edit = edit.clone();
  Arc::new(move || {
    let Some(CommandPermission::Allowed(list)) = edit.draft.get_untracked().0.map(|draft| draft.commands().clone())
    else {
      return;
    };
    match with_command(&list, &edit.new_command.get_untracked()) {
      Ok(list) => {
        edit.change(|draft| draft.set_commands(CommandPermission::Allowed(list)));
        edit.new_command.set(String::new());
        edit.command_error.set(None);
        edit.adding.set(false);
      }
      Err(message) => edit.command_error.set(Some(message)),
    }
  })
}

/// "pnpm build added for this run" (`ecfgf`), and the profile's commands this run leaves out.
fn changed_note(ctx: &Ctx, list: &AllowedCommands, props: &RunPermissionsProps) -> Vec<Element> {
  let profile = profile_commands(props);
  let separator = ctx.t("common.listSeparator");
  let join = |commands: Vec<&str>| commands.join(&separator);
  let added: Vec<&str> = list
    .entries()
    .iter()
    .filter(|entry| !profile.allows(entry))
    .map(|entry| entry.as_str())
    .collect();
  let removed: Vec<&str> = profile
    .entries()
    .iter()
    .filter(|entry| !list.allows(entry))
    .map(|entry| entry.as_str())
    .collect();
  let mut notes = Vec::new();
  if !added.is_empty() {
    notes.push(ctx.t_args("runs.permissions.popover.added", [("commands", join(added).as_str())]));
  }
  if !removed.is_empty() {
    notes.push(ctx.t_args(
      "runs.permissions.popover.removed",
      [("commands", join(removed).as_str())],
    ));
  }
  notes.iter().map(|note| hint(note)).collect()
}

/// A note under a setting, in the style of the unavailable reasons (`W29Od2`).
fn hint(note: &str) -> Element {
  Text::new(note)
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .into()
}

/// Network (`q4Nelz`): a switch, locked on with the reason when off cannot be enforced.
fn network(ctx: &Ctx, edit: &Edit, provider: Option<AgentProvider>, draft: &AgentPermissions) -> Element {
  let network = draft.network();
  let off = reason(ctx, provider, PermissionOption::NetworkOff);
  let on = reason(ctx, provider, PermissionOption::NetworkOn);
  let label = match (network, off.is_some()) {
    (NetworkAccess::Off, false) => ctx.t("agents.permissions.networkOff"),
    (NetworkAccess::Off, true) => ctx.t("agents.permissions.compact.networkStaysOn"),
    (NetworkAccess::On, _) => ctx.t("agents.permissions.networkOn"),
  };
  let shown_on = network == NetworkAccess::On || off.is_some();
  let switch = Switch::new("task-permissions-network", label, shown_on).emphasized();
  let mut content = Vec::new();
  if off.is_none() && on.is_none() {
    let edit = edit.clone();
    let flipped = match network {
      NetworkAccess::Off => NetworkAccess::On,
      NetworkAccess::On => NetworkAccess::Off,
    };
    content.push(switch.render(move || edit.change(|draft| draft.set_network(flipped))));
  } else {
    content.push(switch.locked());
    content.extend(reasons([off, on]));
  }
  group(ctx, "agents.permissions.network", content)
}
