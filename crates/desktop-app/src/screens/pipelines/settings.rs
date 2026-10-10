//! Pipeline settings is a page replacing the canvas (x4IIjM).
//! The design's proposals D0hfv remain visibly unavailable, rather than storing settings runners ignore.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
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
  for key in [
    "pipelines.runDefaults",
    "pipelines.failureDefaults",
    "pipelines.permissions",
    "pipelines.sources",
    "pipelines.delivery",
    "pipelines.archive",
  ] {
    form = form.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .ref_element(sections.target(key))
        .spacing(SpacingSize::Md)
        .child(parts::section(ctx, key))
        .child(parts::note(&ctx.t("pipelines.proposal"))),
    );
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
  Row::new()
    .id("pipeline-settings-page")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(navigation)
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
      .flex(1.0),
    )
    .into()
}

struct General {
  name: Signal<String>,
  slug: Signal<String>,
  description: Signal<String>,
}
impl Component for General {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      description: ctx.signal(ctx.props::<SummaryProps>().state.get().shown().description().to_owned()),
      name: ctx.signal(ctx.props::<SummaryProps>().state.get().name.to_string()),
      slug: ctx.signal(ctx.props::<SummaryProps>().state.get().opened.slug.to_string()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let locked = props.locked;
    self.watch(ctx, &props);
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
      control = control.child(parts::caption(&ctx.t("pipelines.changed"))).child(
        ActionButton::new(
          "pipeline-description-undo",
          ButtonKind::Ghost,
          ctx.t("pipelines.undoChange"),
        )
        .on_click(move || undo.set(before.clone())),
      );
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
      (Icon::Zap, ctx.t("pipelines.onStart")),
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
        ctx.t("pipelines.plannedTrigger"),
        false,
        || {},
      )
      .unavailable(ctx.t("pipelines.planned")),
    );
  }
  form_section::section(
    "pipeline-settings-starts",
    &ctx.t("pipelines.starts"),
    None,
    vec![form_section::side_row(
      Label::new("pipeline-settings-trigger", ctx.t("pipelines.starts")).description(ctx.t("pipelines.startsHelp")),
      choices,
      Place::Last,
      true,
    )],
  )
  .into()
}
impl General {
  fn watch(&self, ctx: &mut Ctx, props: &SummaryProps) {
    let locked = props.locked;
    let rename = props.state.clone();
    ctx.watch(&self.name, move |text| {
      if !locked {
        let name = orchester_workspace::pipelines::fields::parse_name(text);
        rename.update(|s| s.field("pipeline-name", name, |s, name| s.name = name));
      }
    });
    let state = props.state.clone();
    ctx.watch(&self.description, move |text| {
      if !locked {
        state.update(|s| s.edit.set_description(text));
      }
    });
  }
  fn identity(&self, ctx: &mut Ctx) -> (Element, Element) {
    let props = ctx.props::<SummaryProps>();
    let mut name =
      FieldInput::new("pipeline-settings-name", ctx.t("pipelines.name"), self.name.clone()).width(Dimension::Px(300.0));
    if props.locked {
      name = name.locked();
    }
    if let Some(error) = props.state.get().errors.get("pipeline-name") {
      name = name.error(crate::ui::message::message_text(ctx, error));
    }
    let name = name.render(ctx);
    let slug = FieldInput::new("pipeline-settings-slug", ctx.t("pipelines.slug"), self.slug.clone())
      .locked()
      .mono()
      .trailing(Icon::Lock)
      .width(Dimension::Px(300.0))
      .render(ctx);

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
    .child(parts::note(&ctx.t("pipelines.settingsHint")))
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
