//! Pipeline settings is a page replacing the canvas (x4IIjM).
//! The design's proposals D0hfv remain visibly unavailable, rather than storing settings runners ignore.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{inspector::SummaryProps, parts, state::State};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    form_section::{self, Label, Place},
    icon::Icon,
    inspector::{PropertyValue, property},
    option_card::OptionCard,
  },
};
const FORM_WIDTH: f32 = 720.0;

pub fn page(ctx: &mut Ctx, state: &State, busy: bool) -> Element {
  ctx.mount_keyed::<Settings>(
    "pipeline-settings-body",
    SummaryProps {
      state: state.clone(),
      locked: busy,
    },
  )
}
struct Settings {
  navigation: super::settings_nav::Sections,
}
impl Component for Settings {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      navigation: super::settings_nav::Sections::new(ctx),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    content(ctx, &props.state, props.locked, &self.navigation)
  }
}
fn content(ctx: &mut Ctx, state: &State, busy: bool, sections: &super::settings_nav::Sections) -> Element {
  let editing = state.get();
  let changed = editing.viewing.is_none()
    && editing
      .opened
      .latest
      .as_ref()
      .is_some_and(|v| v.body.description() != editing.edit.description());
  let navigation = sections.navigation(ctx, state, changed);
  let description = ctx.mount_keyed::<General>(
    &format!("pipeline-settings-general-{}", editing.epoch),
    SummaryProps {
      state: state.clone(),
      locked: busy || editing.viewing.is_some(),
    },
  );
  let mut form = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(FORM_WIDTH))
    .spacing(ExtraSpacing::Roomy.role())
    .child(heading(ctx, changed, &editing))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .ref_element(sections.target("pipelines.general"))
        .child(description),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .ref_element(sections.target("pipelines.starts"))
        .child(starts(ctx)),
    );
  let locked = busy || editing.viewing.is_some();
  form = proposals(ctx, sections, (state, locked), form);
  Row::new()
    .id("pipeline-settings-page")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(navigation)
    .child(
      Column::new()
        .flex(1.0)
        .height(Dimension::Pct(100.0))
        .with_children(changed.then(|| draft_strip(ctx, state, busy)))
        .child(
          ScrollVertical::new(
            Column::new()
              .width(Dimension::Pct(100.0))
              .padding(ExtraSpacing::Roomy.role())
              .align_items(Alignment::Center)
              .ref_element(sections.origin.clone())
              .child(form),
          )
          .with_scroll_state(sections.scroll.clone())
          .flex(1.0)
          .min_height(Dimension::Px(0.0)),
        ),
    )
    .into()
}

struct General {
  name: Signal<String>,
  slug: Signal<String>,
  description: Signal<String>,
  /// Whether the fields are locked, as last shown.
  locked: Ref<bool>,
}
impl Component for General {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    let general = Self {
      description: ctx.signal(ctx.props::<SummaryProps>().state.get().shown().description().to_owned()),
      name: ctx.signal({
        let editing = ctx.props::<SummaryProps>().state.get();
        editing
          .inputs
          .get("pipeline-name")
          .cloned()
          .unwrap_or_else(|| editing.name.to_string())
      }),
      slug: ctx.signal(ctx.props::<SummaryProps>().state.get().opened.slug.to_string()),
      locked: ctx.create_ref(ctx.props::<SummaryProps>().locked),
    };
    let state = ctx.props::<SummaryProps>().state.clone();
    super::inspector::follow_identity(ctx, &state, (&general.name, &general.description), &general.locked);
    general
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let locked = props.locked;
    self.locked.set(locked);
    let input = FieldInput::new(
      "pipeline-settings-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .text_area()
    .width(Dimension::Px(300.0));
    let input = if locked { input.locked() } else { input }.render(ctx);
    let (name, slug) = self.identity(ctx);
    let changed = editing.viewing.is_none()
      && editing
        .opened
        .latest
        .as_ref()
        .is_some_and(|v| v.body.description() != editing.edit.description());
    let mut control = Column::new()
      .width(Dimension::Px(300.0))
      .spacing(SpacingSize::Sm)
      .child(input);
    if changed {
      let undo = self.description.clone();
      let before = editing
        .opened
        .latest
        .as_ref()
        .map(|v| v.body.description().to_owned())
        .unwrap_or_default();
      let action = ActionButton::new(
        "pipeline-description-undo",
        ButtonKind::Secondary,
        ctx.t("pipelines.undoChange"),
      )
      .on_click(move || undo.set(before.clone()));
      control = control
        .child(changed_tag(ctx, &editing))
        .child(if locked { action.disabled() } else { action });
    }
    form_section::section(
      "pipeline-settings-general",
      &ctx.t("pipelines.general"),
      changed.then(|| ctx.t("pipelines.changed")).as_deref(),
      vec![
        form_section::side_row(
          Label::new("pipeline-settings-name", ctx.t("pipelines.name")).description(ctx.t("pipelines.nameHelp")),
          name,
          Place::Within,
          true,
        ),
        form_section::side_row(
          Label::new("pipeline-settings-slug", ctx.t("pipelines.slug")).description(ctx.t("pipelines.slugHelp")),
          slug,
          Place::Within,
          true,
        ),
        form_section::side_row(
          Label::new("pipeline-settings-description", ctx.t("pipelines.description"))
            .description(ctx.t("pipelines.descriptionHelp")),
          control,
          Place::Last,
          true,
        ),
      ],
    )
  }
}
fn starts(ctx: &Ctx) -> Element {
  let mut choices = Column::new()
    .width(Dimension::Px(300.0))
    .spacing(SpacingSize::Sm)
    .child(OptionCard::new(
      "pipeline-on-start",
      (Icon::Zap, ctx.t("pipelines.onTaskStart")),
      ctx.t("pipelines.onStartHelp"),
      true,
      || {},
    ));
  for (id, title, glyph) in [
    ("pipeline-scheduled", "pipelines.scheduled", Icon::Clock),
    (
      "pipeline-github-trigger",
      "pipelines.githubTrigger",
      Icon::GitPullRequest,
    ),
  ] {
    choices = choices.child(
      OptionCard::new(
        id,
        (glyph, ctx.t(title)),
        ctx.t(if id == "pipeline-scheduled" {
          "pipelines.scheduledHelp"
        } else {
          "pipelines.githubTriggerHelp"
        }),
        false,
        || {},
      )
      .unavailable(ctx.t("pipelines.plannedTrigger")),
    );
  }
  form_section::section(
    "pipeline-settings-starts",
    &ctx.t("pipelines.starts"),
    None,
    vec![form_section::side_row(
      Label::new("pipeline-settings-trigger", ctx.t("pipelines.startsRun")).description(ctx.t("pipelines.startsHelp")),
      choices,
      Place::Last,
      true,
    )],
  )
  .into()
}
impl General {
  fn identity(&self, ctx: &mut Ctx) -> (Element, Element) {
    let props = ctx.props::<SummaryProps>().clone();
    let mut name =
      FieldInput::new("pipeline-settings-name", ctx.t("pipelines.name"), self.name.clone()).width(Dimension::Px(300.0));
    if props.locked {
      name = name.locked();
    }
    if let Some(error) = props.state.get().errors.get("pipeline-name") {
      name = name.error(parts::pipeline_name_error(ctx, error));
    }
    let name = name.render(ctx);
    let slug = FieldInput::new("pipeline-settings-slug", ctx.t("pipelines.slug"), self.slug.clone())
      .locked()
      .mono()
      .trailing(Icon::Lock)
      .width(Dimension::Px(300.0))
      .render(ctx);

    let slug = Column::new()
      .width(Dimension::Px(300.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(slug)
      .child(crate::ui::unavailable::reason_line(
        "pipeline-slug-fixed",
        &if props.state.get().opened.latest.is_some() {
          ctx.t("pipelines.slugPublished")
        } else {
          ctx.t("pipelines.slugFixed")
        },
      ))
      .into();
    (name, slug)
  }
}
fn heading(ctx: &Ctx, changed: bool, editing: &super::state::Editing) -> Column {
  let status = if let Some(version) = editing.viewing_version {
    ctx.t_args("pipelines.viewing", [("version", version.get().to_string())])
  } else {
    ctx.t(if changed {
      "pipelines.draftStrip"
    } else if editing.opened.latest.is_none() {
      "pipelines.saved"
    } else {
      "pipelines.settingsUnchanged"
    })
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("pipelines.settings")).variant(TypographyStyle::Heading))
    .child(parts::note(
      &ctx.t_args(
        "pipelines.settingsDescription",
        [
          ("name", editing.name.to_string()),
          (
            "version",
            orchester_workspace::pipelines::write::next_version(&editing.opened)
              .map_or_else(String::new, |v| v.get().to_string()),
          ),
        ],
      ),
    ))
    .child(
      Row::new()
        .id("pipeline-settings-draft")
        .spacing(SpacingSize::Sm)
        .padding(SpacingSize::Xs)
        .child(crate::ui::icon::icon(
          Icon::GitBranch,
          crate::theme::IconSize::Sm,
          lurq::app::theme::PaletteColor::TextMuted,
        ))
        .child(parts::caption(&status)),
    )
}

fn draft_strip(ctx: &Ctx, state: &State, locked: bool) -> Element {
  let editing = state.get();
  let undo = state.clone();
  let version = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  let action = ActionButton::new(
    "pipeline-settings-undo",
    ButtonKind::Secondary,
    ctx.t("pipelines.undoChange"),
  )
  .icon(Icon::Undo2)
  .on_click(move || {
    undo.update(|s| {
      if let Some(latest) = &s.opened.latest {
        s.edit.set_description(latest.body.description());
        s.epoch += 1;
      }
    })
  });
  Row::new()
    .id("pipeline-settings-change-strip")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(44.0))
    .padding(lurq::node::padding::Padding::horizontal(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(lurq::app::theme::PaletteColor::SurfaceRaised)
    .child(crate::ui::icon::icon(
      Icon::FilePen,
      crate::theme::IconSize::Md,
      lurq::app::theme::PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&ctx.t_args("pipelines.settingsChangeCount", [("version", version)]))
        .variant(crate::theme::ExtraTypography::Strong.role()),
    )
    .child(parts::note(&ctx.t("pipelines.settingsImpact")).flex(1.0))
    .child(if locked { action.disabled() } else { action })
    .into()
}
/// The sections still planned, and Sources (`Q10UQ`) and Delivery (`EYko8`, what a run delivers) among them in the
/// design's order.
fn proposals(
  ctx: &mut Ctx,
  sections: &super::settings_nav::Sections,
  (state, locked): (&State, bool),
  mut form: Column,
) -> Column {
  for key in [
    "pipelines.runDefaults",
    "pipelines.failureDefaults",
    "pipelines.permissions",
    "pipelines.sources",
    "pipelines.delivery",
    "pipelines.archive",
  ] {
    let section = Column::new()
      .width(Dimension::Pct(100.0))
      .ref_element(sections.target(key));
    form = form.child(match key {
      "pipelines.sources" => section.child(super::sources::settings_section(ctx, state, locked)),
      "pipelines.delivery" => section.child(super::delivers::settings_section(ctx, state, locked)),
      _ => section
        .spacing(SpacingSize::Md)
        .child(parts::section(ctx, key))
        .child(parts::note(&ctx.t("pipelines.proposal"))),
    });
  }
  form = form.child(
    property(
      "pipeline-scope",
      &ctx.t("pipelines.who"),
      &ctx.t("pipelines.scope"),
      PropertyValue::Plain,
    )
    .ref_element(sections.target("pipelines.who")),
  );
  form
}
fn changed_tag(ctx: &Ctx, editing: &super::state::Editing) -> Row {
  Row::new()
    .padding(lurq::node::padding::Padding::symmetric(
      SpacingSize::Sm,
      ExtraSpacing::Tight.role(),
    ))
    .rounded(crate::theme::ExtraRadius::Full.role())
    .background(lurq::app::theme::PaletteColor::InfoMuted)
    .child(
      parts::caption(
        &ctx.t_args(
          "pipelines.changedDraft",
          [(
            "version",
            orchester_workspace::pipelines::write::next_version(&editing.opened)
              .map_or_else(String::new, |v| v.get().to_string()),
          )],
        ),
      )
      .color(crate::theme::ExtraColor::Selection.role()),
    )
}
