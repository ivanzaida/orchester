//! *Delete this record?*, in the record's place (`Question · Delete this record?` `n2UhqG` in 05c `CF0R7`), in the
//! hub's danger tone ([`crate::screens::hub::question`]): what deleting does (it stops reaching agents, the
//! invocations that were given it show *Deleted record*, its earlier versions go with it, it cannot be undone), that
//! deprecating keeps it, then *Delete record*, *Deprecate instead* and *Cancel*. Only a person deletes records.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{Memory, MemoryStatus};
use orchester_workspace::{
  background,
  memory::{change::Change, read::GivenTo},
  messages::{self, Message},
};

use super::{state::MemoryState, work::Write};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub struct DeleteProps {
  pub memory: Memory,
  /// How many earlier versions go with it.
  pub earlier: usize,
}

impl DevtoolsInspectable for DeleteProps {}

pub struct DeleteConfirm;

impl Component for DeleteConfirm {
  type Props = DeleteProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeleteProps>().clone();
    let state = MemoryState::of(ctx);
    let memory = &props.memory;
    let id = format!("memory-delete-{}", memory.id());
    let given = given_count(ctx, &state, memory);
    let mut confirm = Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, HubSpacing::Row.role()))
      .spacing(ExtraSpacing::Loose.role())
      .background(PaletteColor::DangerMuted)
      .border_inside(BorderSize::Sm, PaletteColor::Danger)
      .rounded(RadiusSize::Lg)
      .describe("weight", "loses")
      .child(
        Text::new(&ctx.t("memory.delete.title"))
          .id(format!("{id}-title"))
          .variant(ExtraTypography::FormSection.role()),
      )
      .child(
        Text::new(&explanation(ctx, given.as_ref().ok().copied(), props.earlier))
          .id(format!("{id}-text"))
          .variant(HubText::Status.role())
          .width(Dimension::Pct(100.0)),
      );
    if memory.status() == MemoryStatus::Active {
      confirm = confirm.child(
        Text::new(&ctx.t("memory.delete.deprecateHint"))
          .id(format!("{id}-hint"))
          .variant(TypographyStyle::Description)
          .width(Dimension::Pct(100.0)),
      );
    }
    if let Err(Some(reason)) = &given {
      confirm = confirm.child(field_error(&format!("{id}-given-error"), &message_text(ctx, reason)));
    }
    confirm = confirm.with_children(state.failure_note(ctx, &id, Some(memory.id())));
    confirm.child(actions(ctx, &state, memory, &id))
  }
}

/// How many invocations were given `memory`; `Err(None)` while it is read, `Err(Some(why))` when it cannot be.
fn given_count(ctx: &mut Ctx, state: &MemoryState, memory: &Memory) -> Result<usize, Option<Message>> {
  let hold = Services::of(ctx).hold;
  let read = ctx
    .future((state.repo.clone(), memory.id()), move |(repo, record)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || GivenTo::load_in(&repo, record).map(|given| given.invocations.len()))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get();
  let pending = read.is_pending();
  match (read.data, read.error) {
    (Some(count), _) => Ok(count),
    (None, Some(error)) if !pending => Err(Some(error)),
    _ => Err(None),
  }
}

/// What deleting does, as complete sentences: who stops getting it, what goes with it, that it is for good.
fn explanation(ctx: &Ctx, given: Option<usize>, earlier: usize) -> String {
  let reach = match given {
    Some(0) => ctx.t("memory.delete.givenNone"),
    Some(1) => ctx.t("memory.delete.givenOne"),
    Some(count) => ctx.t_args("memory.delete.givenOther", [("count", count.to_string())]),
    None => ctx.t("memory.delete.givenUnknown"),
  };
  let history = match earlier {
    0 => ctx.t("memory.delete.earlierNone"),
    1 => ctx.t("memory.delete.earlierOne"),
    count => ctx.t_args("memory.delete.earlierOther", [("count", count.to_string())]),
  };
  [reach, history, ctx.t("memory.delete.final")].join(" ")
}

/// *Delete record*, *Deprecate instead* on an active record, and *Cancel*, at the start of one row (`m05a8`).
fn actions(ctx: &Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Row {
  let (record, revision) = (memory.id(), memory.revision());
  let busy = state.busy();
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let delete = state.clone();
  let confirm = ActionButton::new(
    format!("{id}-confirm"),
    ButtonKind::Danger,
    ctx.t("memory.delete.confirm"),
  )
  .icon(Icon::Trash2)
  .on_click(move || delete.write(Write::Delete(record, revision)));
  let confirm = if state.pending_for(Some(record)) {
    confirm.loading()
  } else {
    held(confirm)
  };
  let mut row = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(confirm);
  if memory.status() == MemoryStatus::Active {
    let deprecate = state.clone();
    row = row.child(held(
      ActionButton::new(
        format!("{id}-deprecate"),
        ButtonKind::Ghost,
        ctx.t("memory.delete.deprecateInstead"),
      )
      .icon(Icon::Archive)
      .on_click(move || deprecate.write(Write::Change(record, revision, Change::Deprecate))),
    ));
  }
  let close = state.clone();
  row.child(held(
    ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("memory.editor.cancel"))
      .on_click(move || close.close()),
  ))
}
