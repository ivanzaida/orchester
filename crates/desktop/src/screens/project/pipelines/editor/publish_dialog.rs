//! Publish confirmation (03.02 `zfjsK`, dialog `jYL1V`): the published version now and the one
//! the draft becomes, what changed since, and that runs already started keep their version.
//! Publish publishes once; while it runs the button is disabled, so a second click cannot
//! publish the next version as well.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{resource::changes, time::Timestamp};

use crate::{
  display,
  messages::Message,
  pipelines::{describe, editor::EditorState},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
};

/// Dialog width and action bar height (`jYL1V`, `M4vhEv`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;

pub fn publish_dialog(
  ctx: &Ctx,
  state: &EditorState,
  open: &Signal<bool>,
  publish: impl Fn() + Send + Sync + 'static,
  pending: bool,
  error: Option<Message>,
) -> Element {
  let draft = state.draft_number().to_string();
  let current = state.latest().map(|latest| latest.number.to_string());
  let explanation = match &current {
    Some(version) => ctx.t_args(
      "pipelines.publish.explanation",
      [("draft", draft.as_str()), ("version", version.as_str())],
    ),
    None => ctx.t_args("pipelines.publish.explanationFirst", [("draft", draft.as_str())]),
  };
  let header = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t_args(
        "pipelines.publish.title",
        [("pipeline", state.name()), ("draft", draft.as_str())],
      ))
      .variant(ExtraTypography::DialogTitle.role()),
    )
    .child(Text::new(&explanation).variant(ExtraTypography::Explanation.role()));
  let mut body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .child(transition(ctx, state, &draft))
    .child(change_list(ctx, state));
  if let Some(version) = &current {
    body = body.child(Notice::new(
      Icon::Pin,
      ctx.t_args("pipelines.publish.keepTitle", [("version", version.as_str())]),
      ctx.t_args("pipelines.publish.keepDetail", [("version", version.as_str())]),
    ));
  }
  Column::new()
    .id("publish-dialog")
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header)
    .child(body)
    .child(actions(ctx, &draft, open, publish, pending, error))
    .into()
}

/// Published now → After publishing (`LoiX6`).
fn transition(ctx: &Ctx, state: &EditorState, draft: &str) -> Element {
  let now = Timestamp::now();
  let (current, current_detail) = match state.latest() {
    Some(latest) => (
      ctx.t_args(
        "pipelines.publish.versionValue",
        [("version", latest.number.to_string().as_str())],
      ),
      ctx.t_args(
        "pipelines.publish.since",
        [
          ("time", &*message_text(ctx, &display::elapsed(now, latest.at))),
          ("tasks", &*message_text(ctx, &describe::task_count(state.used_by()))),
        ],
      ),
    ),
    None => (
      ctx.t("pipelines.publish.noneYet"),
      ctx.t("pipelines.publish.noneYetDetail"),
    ),
  };
  let saved = state.draft_saved_at().map(|at| display::elapsed(now, at));
  let after_detail = match saved.filter(|elapsed| elapsed.key != "time.justNow") {
    Some(elapsed) => ctx.t_args(
      "pipelines.publish.fromDraft",
      [("draft", draft), ("time", &*message_text(ctx, &elapsed))],
    ),
    None => ctx.t_args("pipelines.publish.fromDraftJustNow", [("draft", draft)]),
  };
  let card = |label: &str, value: &str, detail: &str, chosen: bool| {
    let (background, border) = if chosen {
      (PaletteColor::SurfaceRaised, PaletteColor::TextPrimary)
    } else {
      (PaletteColor::SurfacePanel, PaletteColor::Border)
    };
    Column::new()
      .flex(1.0)
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Tight.role())
      .background(background)
      .border(Border::inside(BorderSize::Sm, border))
      .rounded(RadiusSize::Md)
      .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Caption))
      .child(Text::new(value).variant(ExtraTypography::VersionFigure.role()))
      .child(Text::new(detail).variant(TypographyStyle::Caption))
  };
  let after = ctx.t_args("pipelines.version.number", [("draft", draft)]);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(card("pipelines.publish.publishedNow", &current, &current_detail, false))
    .child(icon(Icon::ArrowRight, IconSize::Option, PaletteColor::TextSecondary))
    .child(card("pipelines.publish.afterPublishing", &after, &after_detail, true))
    .into()
}

/// Changes from the published version (`LXUS6`), or that this is the first version.
fn change_list(ctx: &Ctx, state: &EditorState) -> Element {
  let Some(latest) = state.latest() else {
    return Text::new(&ctx.t("pipelines.publish.firstVersion"))
      .variant(TypographyStyle::Caption)
      .into();
  };
  let Ok(draft) = state.edit().validate() else {
    return Column::new().into();
  };
  let found = changes(&latest.body, &draft);
  let version = latest.number.to_string();
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(
      Text::new(&ctx.t_args("pipelines.publish.changes", [("version", version.as_str())]))
        .variant(TypographyStyle::Caption),
    );
  if found.is_empty() {
    list = list.child(
      Text::new(&ctx.t("pipelines.publish.noChanges"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    );
  }
  for change in &found {
    list = list.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Pencil, IconSize::Small, PaletteColor::TextMuted))
        .child(
          Text::new(&message_text(ctx, &describe::change(change)))
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        ),
    );
  }
  list.into()
}

fn actions(
  ctx: &Ctx,
  draft: &str,
  open: &Signal<bool>,
  publish: impl Fn() + Send + Sync + 'static,
  pending: bool,
  error: Option<Message>,
) -> Element {
  let close = open.clone();
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()));
  row = match error {
    Some(error) => row.child(Column::new().flex(1.0).child(field_error(&message_text(ctx, &error)))),
    None => row.child(Spacer::new().flex(1.0)),
  };
  row
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .icon(Icon::X)
        .id("publish-cancel")
        .on_click(move || close.set(false))
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t_args("pipelines.editor.publish", [("draft", draft)]))
        .primary()
        .icon(Icon::Check)
        .id("publish-confirm")
        .on_click(publish)
        .enabled(!pending)
        .render(ctx),
    )
    .into()
}
