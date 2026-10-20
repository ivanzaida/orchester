//! *Delete this record?*, in the record's place (`Memory / Delete confirmation` `YdVse`): what deleting does (it
//! stops reaching agents, the invocations that were given it show *Deleted record*, its earlier versions go with it,
//! it cannot be undone), the record as it is with how many invocations were given it, that deprecating keeps it, and
//! *Deprecate instead*, *Cancel* and *Delete record*. Only a person deletes records.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{Memory, MemoryStatus};
use orchester_workspace::{
  background,
  memory::{change::Change, read::GivenTo},
  messages::{self, Message},
};

use super::{state::MemoryState, words, work::Write};
use crate::{
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, MemoryText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub struct DeleteProps {
  pub memory: Memory,
  /// Who wrote it and where, as its meta line starts.
  pub written: Arc<str>,
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
    let kind = words::kind_name(ctx, memory.content().kind());
    let what = match memory.content().key() {
      Some(key) => words::join(ctx, &[kind.to_string(), key.as_str().to_owned()]),
      None => kind,
    };
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Trash2, IconSize::Md, PaletteColor::Danger))
      .child(Text::new(&ctx.t("memory.delete.title")).variant(ExtraTypography::FormSection.role()))
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&what).variant(TypographyStyle::Caption).nowrap());
    let mut confirm = Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .child(head)
      .child(
        Text::new(&explanation(ctx, given.as_ref().ok().copied(), props.earlier))
          .id(format!("{id}-text"))
          .variant(TypographyStyle::Description),
      )
      .child(preview(ctx, &props, given.as_ref().ok().copied(), &id));
    if memory.status() == MemoryStatus::Active {
      confirm = confirm.child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Archive, IconSize::Sm, PaletteColor::TextMuted))
          .child(Text::new(&ctx.t("memory.delete.deprecateHint")).variant(MemoryText::NoticeText.role())),
      );
    }
    if let Err(Some(reason)) = &given {
      confirm = confirm.child(field_error(&format!("{id}-given-error"), &message_text(ctx, reason)));
    }
    if let Some(failed) = state.failure_for(Some(memory.id())) {
      confirm = confirm.child(field_error(&format!("{id}-error"), &message_text(ctx, &failed)));
    }
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

/// The record as it is (`I6Sjzs`): its key, what it says, and who wrote it with how many invocations got it.
fn preview(ctx: &Ctx, props: &DeleteProps, given: Option<usize>, id: &str) -> Column {
  let memory = &props.memory;
  let mut meta = vec![props.written.to_string()];
  if let Some(count) = given {
    meta.push(if count == 1 {
      ctx.t("memory.delete.givenToOne").to_string()
    } else {
      ctx
        .t_args("memory.delete.givenTo", [("count", count.to_string())])
        .to_string()
    });
  }
  Column::new()
    .id(format!("{id}-preview"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Xs)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .with_children(
      memory
        .content()
        .key()
        .map(|key| Text::new(key.as_str()).variant(ExtraTypography::MonoNote.role())),
    )
    .child(
      Text::new(memory.content().text())
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(Text::new(&words::join(ctx, &meta)).variant(TypographyStyle::Caption))
}

/// *Deprecate instead* on an active record, *Cancel* and *Delete record* (`bQ3eS`).
fn actions(ctx: &Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Row {
  let (record, revision) = (memory.id(), memory.revision());
  let busy = state.busy();
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  if memory.status() == MemoryStatus::Active {
    let deprecate = state.clone();
    row = row.child(held(
      ActionButton::new(
        format!("{id}-deprecate"),
        ButtonKind::Secondary,
        ctx.t("memory.delete.deprecateInstead"),
      )
      .icon(Icon::Archive)
      .on_click(move || deprecate.write(Write::Change(record, revision, Change::Deprecate))),
    ));
  }
  let close = state.clone();
  let cancel = held(
    ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("memory.editor.cancel"))
      .on_click(move || close.close()),
  );
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
  row.child(Spacer::new().flex(1.0)).child(cancel).child(confirm)
}
