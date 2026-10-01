//! The Permissions tab (04.04 `T1rjv`): repository access, extra writable folders (`folders`), commands, network and
//! MCP servers (`mcp`), each option
//! disabled with the reason when the chosen connection cannot enforce it (04.B `W0Owo`), beside
//! the panel that says how the connection enforces them (`enforcement`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, Support, support},
  },
};

use super::editor::{ProfileEditor, Resets};
use crate::{
  agents::permissions::{
    file_command_hints, native_unavailable, option_detail, option_title, unavailable, with_command, without_command,
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    command_chip::CommandChip,
    field::{field_error, text_input},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
    option_card::{OptionCard, unavailable_reason},
    switch::Switch,
  },
};

/// Command input width and the height of the entries beside it (`mwPz8`, `bpmzN`).
const COMMAND_INPUT_WIDTH: f32 = 180.0;
const COMMAND_HEIGHT: f32 = 26.0;

impl ProfileEditor {
  pub(super) fn permissions_tab(&self, ctx: &Ctx) -> Vec<Element> {
    let role = self.role.get();
    let role = if role.trim().is_empty() {
      ctx.t("agents.newTitle")
    } else {
      role.trim().into()
    };
    let provider = self.selected_connection().map(|connection| connection.provider());
    let settings = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Form.role())
      // Option cards are focus stops; their outer rings reach past the column.
      .overflow_visible()
      .child(Text::new(&ctx.t_args("agents.permissions.intro", [("role", &*role)])).variant(TypographyStyle::FieldLabel))
      .child(self.repository_setting(ctx, provider))
      .child(self.folders_setting(ctx))
      .child(self.commands_setting(ctx, provider))
      .child(self.network_setting(ctx, provider))
      .child(self.tool_approvals_setting(ctx, provider))
      .child(self.mcp_setting(ctx, provider));
    let body = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Section)
      .align_items(Alignment::Start)
      .overflow_visible()
      .child(settings)
      .child(self.enforcement_panel(ctx));
    vec![
      body.into(),
      Notice::new(
        Icon::History,
        ctx.t("agents.permissions.appliesTitle"),
        ctx.t("agents.permissions.appliesDetail"),
      )
      .into(),
    ]
  }

  fn tool_approvals_setting(&self, ctx: &Ctx, provider: Option<AgentProvider>) -> Element {
    let enabled = self.permissions.get().interactive_tool_approvals();
    let switch = Switch::new(
      "agent-tool-approvals",
      ctx.t("agents.permissions.toolApprovals"),
      enabled,
    );
    // Both CLIs route approvals to Orchester; a profile without a connection cannot choose, nor can
    // one on a native connection, which runs no agent yet.
    let native = native_unavailable(ctx, provider);
    let control = if native.is_none() && (provider.is_some() || enabled) {
      let permissions = self.permissions.clone();
      switch.render(move || permissions.update(|permissions| permissions.set_interactive_tool_approvals(!enabled)))
    } else {
      switch.locked()
    };
    let content = Column::new()
      .spacing(SpacingSize::Sm)
      .child(control)
      .with_children(native.map(|reason| unavailable_reason(&reason)));
    setting(ctx, "agents.permissions.toolApprovalsHeading", content.into())
  }

  /// An option card for `option`, chosen by `choose`, or disabled with the reason when the
  /// connection cannot enforce it.
  fn option(
    &self,
    ctx: &Ctx,
    provider: Option<AgentProvider>,
    option: PermissionOption,
    selected: bool,
    choose: impl Fn(&mut AgentPermissions) + Send + Sync + 'static,
  ) -> Element {
    let card = OptionCard::new(
      option_id(option),
      ctx.t(option_title(option)),
      ctx.t(option_detail(option, provider)),
      selected,
    );
    let Some(provider) = provider else {
      return card.unavailable(None).into();
    };
    match support(provider, Platform::current(), option) {
      Support::Enforced(_) => {
        let (permissions, resets) = (self.permissions.clone(), self.resets.clone());
        card
          .on_choose(move || {
            permissions.update(|permissions| choose(permissions));
            resets.set(Resets::default());
          })
          .into()
      }
      Support::Unavailable(reason) => card.unavailable(unavailable(ctx, reason, provider)).into(),
    }
  }

  fn repository_setting(&self, ctx: &Ctx, provider: Option<AgentProvider>) -> Element {
    let current = self.permissions.get().repository();
    let option = |option, access| {
      Column::new().flex(1.0).overflow_visible().child(self.option(
        ctx,
        provider,
        option,
        current == access,
        move |permissions| permissions.set_repository(access),
      ))
    };
    setting(
      ctx,
      "agents.permissions.repository",
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Start)
        .overflow_visible()
        .child(option(PermissionOption::ReadOnly, RepositoryAccess::ReadOnly))
        .child(option(PermissionOption::EditWorktree, RepositoryAccess::Write))
        .into(),
    )
  }

  fn commands_setting(&self, ctx: &Ctx, provider: Option<AgentProvider>) -> Element {
    let commands = self.permissions.get().commands().clone();
    let listed = match &commands {
      CommandPermission::Allowed(list) => Some(list.clone()),
      CommandPermission::AskBefore | CommandPermission::AnyInWorktree => None,
    };
    let kept = listed.clone().unwrap_or_default();
    let column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .overflow_visible()
      .child(self.option(
        ctx,
        provider,
        PermissionOption::AskBeforeCommands,
        commands == CommandPermission::AskBefore,
        |permissions| permissions.set_commands(CommandPermission::AskBefore),
      ))
      .child(self.option(
        ctx,
        provider,
        PermissionOption::AllowedCommands,
        listed.is_some(),
        move |permissions| permissions.set_commands(CommandPermission::Allowed(kept.clone())),
      ))
      .with_children(listed.map(|list| self.command_list(ctx, &list)))
      .child(self.option(
        ctx,
        provider,
        PermissionOption::AnyCommand,
        commands == CommandPermission::AnyInWorktree,
        |permissions| permissions.set_commands(CommandPermission::AnyInWorktree),
      ));
    setting(ctx, "agents.permissions.commands", column.into())
  }

  /// The allowed commands as entries that remove themselves, a field to add one, and how they
  /// match (`AJ0IR`). Enter in the field adds the command.
  fn command_list(&self, ctx: &Ctx, list: &AllowedCommands) -> Element {
    let entries = list.entries().iter().enumerate().map(|(index, entry)| {
      let (permissions, removed) = (self.permissions.clone(), entry.clone());
      Element::from(
        CommandChip::new(format!("agent-command-{index}"), entry.as_str().into()).on_remove(move || {
          permissions.update(|permissions| {
            if let CommandPermission::Allowed(list) = permissions.commands() {
              let rest = without_command(list, &removed);
              permissions.set_commands(CommandPermission::Allowed(rest));
            }
          });
        }),
      )
    });
    let add = self.add_command();
    let input = text_input(
      ctx,
      "agent-new-command",
      self.new_command.clone(),
      self.command_error.get().is_some(),
      ExtraTypography::MonoValue.role(),
    )
    .size(Dimension::Px(COMMAND_INPUT_WIDTH), Dimension::Px(COMMAND_HEIGHT))
    .placeholder(&ctx.t("agents.permissions.commandPlaceholder"));
    let add_button = ActionButton::new(ctx.t("agents.permissions.addCommand"))
      .icon(Icon::Plus)
      .compact()
      .id("agent-add-command")
      .in_form()
      .height(Dimension::Px(COMMAND_HEIGHT));
    let add_button = match &add {
      Some(add) => {
        let add = add.clone();
        add_button.on_click(move || add())
      }
      None => add_button.on_click(|| {}).enabled(false),
    };
    let entries = Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .overflow_visible()
      .with_children(entries)
      .child(input)
      .child(add_button.render(ctx));
    let note: Element = match self.command_error.get() {
      Some(error) => field_error(&message_text(ctx, &error)),
      None if list.is_empty() => hint(ctx, "agents.permissions.noCommands"),
      None => hint(ctx, "agents.permissions.commandMatching"),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_left(ExtraSpacing::Rail.role())
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      // Enter in the command field adds the command rather than saving the draft.
      .child(form(add, entries))
      .child(note)
      .with_children(file_command_hints(ctx, list).iter().map(|text| hint_text(text)))
      .into()
  }

  /// Adds the typed command to the allowed commands, or says why it cannot be added; `None` while
  /// nothing is typed.
  fn add_command(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
    if self.new_command.get().trim().is_empty() {
      return None;
    }
    let (typed, permissions, error) = (
      self.new_command.clone(),
      self.permissions.clone(),
      self.command_error.clone(),
    );
    Some(Arc::new(move || {
      let CommandPermission::Allowed(list) = permissions.get_untracked().commands().clone() else {
        return;
      };
      match with_command(&list, &typed.get_untracked()) {
        Ok(list) => {
          permissions.update(|permissions| permissions.set_commands(CommandPermission::Allowed(list)));
          typed.set(String::new());
          error.set(None);
        }
        Err(message) => error.set(Some(message)),
      }
    }))
  }

  /// The network switch (`k1QWD`), locked on with the reason when off cannot be enforced
  /// (`lW7Sp`). It shows the network access a run gets, which is on wherever off cannot be
  /// enforced, whatever was chosen. On a native connection, where no run starts, it shows the chosen
  /// access, locked with the reason, as the permission summary does.
  fn network_setting(&self, ctx: &Ctx, provider: Option<AgentProvider>) -> Element {
    let chosen = self.permissions.get();
    let network = provider.map_or(chosen.network(), |provider| {
      chosen.effective_network(provider, Platform::current())
    });
    let off = provider.map(|provider| {
      (
        provider,
        support(provider, Platform::current(), PermissionOption::NetworkOff),
      )
    });
    let (label, detail) = match network {
      NetworkAccess::Off => ("agents.permissions.networkOff", "agents.permissions.networkOffDetail"),
      NetworkAccess::On => ("agents.permissions.networkOn", "agents.permissions.networkOnDetail"),
    };
    let switch = Switch::new("agent-network", ctx.t(label), network == NetworkAccess::On).emphasized();
    let mut column = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
    column = match off {
      Some((_, Support::Enforced(_))) => {
        let permissions = self.permissions.clone();
        let flipped = match network {
          NetworkAccess::Off => NetworkAccess::On,
          NetworkAccess::On => NetworkAccess::Off,
        };
        column.child(
          switch
            .detail(ctx.t(detail))
            .render(move || permissions.update(|permissions| permissions.set_network(flipped))),
        )
      }
      Some((provider, Support::Unavailable(reason))) => column
        .child(switch.locked())
        .with_children(unavailable(ctx, reason, provider).map(|reason| unavailable_reason(&reason))),
      None => column.child(switch.locked()),
    };
    setting(ctx, "agents.permissions.network", column.into())
  }
}

/// A setting's label over its control (`w69VK5`).
fn setting(ctx: &Ctx, label: &str, control: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .overflow_visible()
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Label))
    .child(control)
    .into()
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  hint_text(&ctx.t(key))
}

fn hint_text(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .into()
}

const fn option_id(option: PermissionOption) -> &'static str {
  match option {
    PermissionOption::ReadOnly => "agent-permission-read-only",
    PermissionOption::EditWorktree => "agent-permission-edit",
    PermissionOption::AskBeforeCommands => "agent-permission-ask",
    PermissionOption::AllowedCommands => "agent-permission-allowed",
    PermissionOption::AnyCommand => "agent-permission-any",
    PermissionOption::NetworkOff => "agent-permission-network-off",
    PermissionOption::NetworkOn => "agent-permission-network-on",
  }
}
