//! The Settings tab (`02d · settings` `A29iv`, `02h · settings · delivery` `Wq6tZ`): General (name, slug,
//! description, who can use it), Runs (what starts them and what happens when a step fails, as plain values),
//! Sources for every step, and Delivery with what a run delivers and the Deliver step. What the draft changed shows in
//! its strip with *Undo change* and on its row (*Changed in draft vN*). The planned sections are gone.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pipelines::write::next_version;

use super::{
  parts,
  state::{Editing, State, View},
  summary::SummaryProps,
};
use crate::{
  theme::{ExtraColor, ExtraTypography, GroupText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    pressable::Press,
    row_group::{GroupRow, RowGroup},
  },
};

/// The name and description fields' width (`Input` `LuF69`).
const FIELD_WIDTH: f32 = 260.0;

pub fn page(ctx: &mut Ctx, state: &State, locked: bool) -> Element {
  let editing = state.get();
  ctx.mount_keyed::<Settings>(
    &format!("pipeline-settings-{}", editing.epoch),
    SummaryProps {
      state: state.clone(),
      locked: locked || editing.viewing.is_some(),
    },
  )
}

struct Settings {
  name: Signal<String>,
  description: Signal<String>,
  /// Whether the fields are locked, as last shown.
  locked: Ref<bool>,
}
impl Component for Settings {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let settings = Self {
      description: ctx.signal(editing.shown().description().to_owned()),
      name: ctx.signal(
        editing
          .inputs
          .get("pipeline-name")
          .cloned()
          .unwrap_or_else(|| editing.name.to_string()),
      ),
      locked: ctx.create_ref(props.locked),
    };
    super::summary::follow_identity(ctx, &props.state, (&settings.name, &settings.description), &settings.locked);
    settings
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    self.locked.set(props.locked);
    let editing = props.state.get();
    let changed = changed(&editing);
    let mut pane = parts::pane("pipeline-settings-page");
    if changed.count() > 0 {
      pane = pane.child(strip(ctx, &props.state, (changed.count(), props.locked)));
    }
    let general = self.general(ctx, &props, &editing, changed.description);
    pane
      .child(general)
      .child(runs(ctx))
      .child(super::sources::settings_section(ctx, &props.state, props.locked))
      .child(super::delivers::section(ctx, &props.state, (props.locked, changed.delivers)))
      .child(deliver_step(ctx, &editing))
  }
}

/// The settings the draft changed from the published version.
#[derive(Clone, Copy)]
struct Changed {
  description: bool,
  delivers: bool,
}
impl Changed {
  const fn count(self) -> usize {
    self.description as usize + self.delivers as usize
  }
}
fn changed(editing: &Editing) -> Changed {
  let latest = editing.opened.latest.as_ref().filter(|_| editing.viewing.is_none());
  Changed {
    description: latest.is_some_and(|v| v.body.description() != editing.edit.description()),
    delivers: latest.is_some_and(|v| v.body.delivers() != editing.edit.delivers()),
  }
}

fn next(editing: &Editing) -> String {
  next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string())
}

impl Settings {
  fn general(&self, ctx: &mut Ctx, props: &SummaryProps, editing: &Editing, changed: bool) -> RowGroup {
    let mut name =
      FieldInput::new("pipeline-settings-name", ctx.t("pipelines.name"), self.name.clone()).width(Dimension::Px(FIELD_WIDTH));
    if props.locked {
      name = name.locked();
    }
    if let Some(error) = editing.errors.get("pipeline-name") {
      name = name.error(parts::pipeline_name_error(ctx, error));
    }
    let name = name.render(ctx);
    let description = FieldInput::new(
      "pipeline-settings-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .width(Dimension::Px(FIELD_WIDTH));
    let description = if props.locked { description.locked() } else { description }.render(ctx);
    let mut description_row = GroupRow::new("pipeline-settings-description-row", ctx.t("pipelines.description"))
      .hint(ctx.t("pipelines.descriptionHelp"))
      .end(description);
    if changed {
      description_row = description_row.tag(draft_tag(ctx, editing));
    }
    let slug_hint = ctx.t(if editing.opened.latest.is_some() {
      "pipelines.slugPublished"
    } else {
      "pipelines.slugFixed"
    });
    RowGroup::new("pipeline-settings-general", ctx.t("pipelines.general"))
      .row(GroupRow::new("pipeline-settings-name-row", ctx.t("pipelines.name")).end(name))
      .row(
        GroupRow::new("pipeline-settings-slug", ctx.t("pipelines.slug"))
          .hint(slug_hint)
          .mono_value(&editing.opened.slug),
      )
      .row(description_row)
      .row(GroupRow::new("pipeline-scope", ctx.t("pipelines.who")).value(&ctx.t("pipelines.scope")))
  }
}

/// *Changed in draft vN* on a changed setting's row (`xeLvf`).
pub(super) fn draft_tag(ctx: &Ctx, editing: &Editing) -> Row {
  Row::new()
    .id("pipeline-settings-draft")
    .padding(Padding::symmetric(SpacingSize::Sm, crate::theme::ExtraSpacing::Tight.role()))
    .rounded(crate::theme::ExtraRadius::Full.role())
    .background(PaletteColor::InfoMuted)
    .child(
      Text::new(&ctx.t_args("pipelines.changedDraft", [("version", next(editing))]))
        .variant(lurq::app::theme::TypographyStyle::Caption)
        .color(ExtraColor::AccentText.role()),
    )
}

fn runs(ctx: &Ctx) -> RowGroup {
  RowGroup::new("pipeline-settings-runs", ctx.t("pipelines.runsTitle"))
    .row(GroupRow::new("pipeline-on-start", ctx.t("pipelines.starts")).value(&ctx.t("pipelines.onTaskStart")))
    .row(GroupRow::new("pipeline-settings-failure", ctx.t("pipelines.stopped")).value(&ctx.t("pipelines.stoppedSummary")))
}

/// The Deliver step, which opens on the Steps tab.
fn deliver_step(ctx: &Ctx, editing: &Editing) -> RowGroup {
  let hint = editing
    .shown()
    .deliver()
    .map_or_else(|| ctx.t("pipelines.noDeliverChip"), |deliver| super::deliver::executor_words(ctx, deliver));
  let views = editing.views.clone();
  RowGroup::new("pipeline-settings-deliver", ctx.t("pipelines.deliverStep")).row(
    GroupRow::new("pipeline-settings-deliver-row", ctx.t("delivery.step.title"))
      .hint(hint)
      .opens_page(Press::run(move || views.choose(View::Deliver))),
  )
}

/// The draft strip (`Draft strip` `uHJyF`): how many settings the draft changed, what that means, *Undo change*.
fn strip(ctx: &Ctx, state: &State, (count, locked): (usize, bool)) -> Element {
  let editing = state.get();
  let undo = state.clone();
  let action = ActionButton::new("pipeline-settings-undo", ButtonKind::Secondary, ctx.t("pipelines.undoChange"))
    .icon(Icon::Undo2)
    .on_click(move || {
      undo.update(|s| {
        if let Some(latest) = s.opened.latest.clone() {
          s.edit.set_description(latest.body.description());
          s.edit.set_delivers(latest.body.delivers());
          s.epoch += 1;
        }
      })
    });
  let title = if count == 1 {
    ctx.t_args("pipelines.settingsChangeCount", [("version", next(&editing))])
  } else {
    ctx.t_args(
      "pipelines.settingsChangesCount",
      [("count", count.to_string()), ("version", next(&editing))],
    )
  };
  Row::new()
    .id("pipeline-settings-change-strip")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(crate::theme::ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(Icon::FilePen, IconSize::Md, ExtraColor::AccentText.role()))
    .child(Text::new(&title).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&ctx.t("pipelines.settingsImpact"))
        .variant(GroupText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(if locked { action.disabled() } else { action })
    .into()
}
