//! The allowed commands under *Allowed commands* (01c): the prefixes as removable chips, then a field and Add
//! command; a prefix is checked by `orchester_workspace::agents::permissions` before it is added.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::permission::CommandPermission;
use orchester_workspace::{agents::permissions, messages::Message};

use super::Context;
use crate::{
  screens::agents::fields::Fields,
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
  },
};

/// The prefix field's width (`e.g. pnpm test` in 01c).
const FIELD_WIDTH: f32 = 250.0;

#[derive(Clone)]
pub struct Props {
  pub fields: Fields,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.fields.permissions.id() == other.fields.permissions.id()
  }
}
impl DevtoolsInspectable for Props {}

pub(super) fn props(context: &Context<'_>) -> Props {
  Props {
    fields: context.fields.clone(),
  }
}

pub struct CommandList {
  typed: Signal<String>,
  error: Signal<Option<Message>>,
}

impl Component for CommandList {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      typed: ctx.signal(String::new()),
      error: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let fields = ctx.props::<Props>().fields.clone();
    let CommandPermission::Allowed(list) = fields.permissions.get().0.commands().clone() else {
      return Column::new();
    };
    let mut chips = Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role());
    for (index, command) in list.entries().iter().enumerate() {
      let (command, signal) = (command.clone(), fields.permissions.clone());
      chips = chips.child(
        TagChip::new(format!("agent-command-{index}"), command.as_str())
          .mono()
          .removable(ctx.t("agents.remove"), move || {
            signal.update(|p| {
              if let CommandPermission::Allowed(list) = p.0.commands() {
                p.0
                  .set_commands(CommandPermission::Allowed(permissions::without_command(list, &command)));
              }
            });
          }),
      );
    }
    let input = FieldInput::new("agent-command", ctx.t("agents.command"), self.typed.clone())
      .placeholder(ctx.t("agents.commandPlaceholder"))
      .mono()
      .width(Dimension::Px(FIELD_WIDTH))
      .render(ctx);
    let (value, typed, error) = (fields.permissions, self.typed.clone(), self.error.clone());
    let add = ActionButton::new("agent-add-command", ButtonKind::Ghost, ctx.t("agents.addCommand"))
      .icon(Icon::Plus)
      .on_click(move || {
        if let CommandPermission::Allowed(list) = value.get().0.commands() {
          match permissions::with_command(list, &typed.get()) {
            Ok(list) => {
              value.update(|p| p.0.set_commands(CommandPermission::Allowed(list)));
              typed.set(String::new());
              error.set(None);
            }
            Err(reason) => error.set(Some(reason)),
          }
        }
      });
    Column::new()
      .id("agent-command-entries")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(HubSpacing::Groups.role())
          .right(HubSpacing::Row.role())
          .bottom(HubSpacing::FieldY.role()),
      )
      .spacing(SpacingSize::Sm)
      .with_children((!list.entries().is_empty()).then_some(chips))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(input)
          .child(add),
      )
      .with_children(
        self
          .error
          .get()
          .map(|error| field_error("agent-command-error", &message_text(ctx, &error))),
      )
      .child(Text::new(&ctx.t("agents.commandNote")).variant(HubText::Hint.role()))
  }
}
