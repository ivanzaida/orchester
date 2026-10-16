//! Inline metadata, Used by links and archive confirmation (DrLE6, K2epbr).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::secret::Secret;
use orchester_workspace::{
  activity::operations,
  secrets::data::{ProjectSecrets, SecretUseKind},
  tasks::outside::Outside,
};

use super::{
  state::{Panel, View},
  words, work,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsTab, ConnectionsView, ProjectPage, Route},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, SecretsSpacing, SecretsText},
  ui::{
    avatar::credential_avatar,
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    inspector,
    message::message_text,
    notice::handling_notice,
    tooltip::Truncated,
  },
};
pub(super) const WIDTH: f32 = 360.0;
/// Masked value DShSq.
const MASK_HEIGHT: f32 = 32.0;

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub listed: ProjectSecrets,
  pub secret: Secret,
  pub view: View,
  pub archive: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.listed == other.listed
      && self.secret == other.secret
      && self.archive == other.archive
  }
}
impl DevtoolsInspectable for Props {}
pub(super) struct Inspector {
  action: work::Action,
}
impl Component for Inspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let action = work::action(ctx);
    let view = ctx.props::<Props>().view.clone();
    let changes = Services::of(ctx).changes;
    let project = ctx.props::<Props>().project.open.project.id();
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() {
        view.panel.set(Panel::Inspect);
        if matches!(state.data, Some(work::Outcome::Saved(_))) {
          // Every screen that shows the project's secrets and connections reads them again, this one too.
          changes.record(project, Outside::Connections);
        } else {
          view.revision.update(|revision| *revision += 1);
        }
      }
    });
    Self { action }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let action = self.action.clone();
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SecretsSpacing::Body.role()))
      .spacing(SecretsSpacing::Body.role())
      .child(metadata(ctx, &props.project, &props.secret, !props.archive))
      .child(uses(ctx, &props.project, &props.listed, &props.secret))
      .with_children((!props.archive).then(|| {
        inspector::section("secret-recent", ExtraSpacing::Snug.role())
          .padding(Padding::new().top(SecretsSpacing::Body.role()))
          .child(inspector::section_title(
            "secret-recent-title",
            &ctx.t("secrets.recent"),
            None,
          ))
          .child(inspector::note(
            "secret-recent-note",
            Icon::Lock,
            &ctx.t("secrets.recentHint"),
          ))
      }));
    frame(
      header(ctx, &props.secret),
      body.into(),
      self.footer(ctx, &props, &action),
    )
  }
}
impl Inspector {
  fn footer(&self, ctx: &mut Ctx, props: &Props, action: &work::Action) -> Element {
    let state = action.state().get();
    let mut footer = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
    if props.secret.is_archived() {
      return footer
        .child(inspector::note(
          "secret-archived",
          Icon::Archive,
          &ctx.t("secrets.archivedHint"),
        ))
        .into();
    }
    if props.archive {
      let title = ctx.t_args("secrets.archiving.title", [("name", props.secret.name().as_str())]);
      let explanation = ctx.t_args(
        "secrets.archiving.explanation",
        [("project", props.project.open.project.name().as_str())],
      );
      footer = footer
        .background(PaletteColor::SurfaceRaised)
        .padding(Padding::symmetric(SpacingSize::Lg, SecretsSpacing::Body.role()))
        .spacing(ExtraSpacing::Loose.role())
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(icon(Icon::Archive, IconSize::Md, PaletteColor::Danger))
            .child(Text::new(&title).variant(SecretsText::ConfirmTitle.role())),
        )
        .child(Text::new(&explanation).variant(ExtraTypography::Prose.role()))
        .child(handling_notice(
          "secret-archive-impact",
          Icon::Unlink,
          &words::archive_impact(ctx, props.listed.use_counts(props.secret.id())),
        ));
    }
    if let Some(error) = &state.error {
      footer = footer.child(field_error("secret-archive-error", &message_text(ctx, error)));
    }
    footer
      .child(self.buttons(ctx, props, action, state.is_pending()))
      .into()
  }
  fn buttons(&self, ctx: &mut Ctx, props: &Props, action: &work::Action, pending: bool) -> Row {
    let panel = props.view.panel.clone();
    let left = if props.archive {
      ActionButton::new("secret-archive-cancel", ButtonKind::Secondary, ctx.t("secrets.cancel"))
        .on_click(move || panel.set(Panel::Inspect))
    } else {
      ActionButton::new("secret-replace", ButtonKind::Secondary, ctx.t("secrets.replace"))
        .icon(Icon::RefreshCw)
        .on_click(move || panel.set(Panel::Replace))
    };
    let right = ActionButton::new(
      "secret-archive",
      if props.archive {
        ButtonKind::Danger
      } else {
        ButtonKind::DangerGhost
      },
      ctx.t(if pending {
        "secrets.archiving.running"
      } else {
        "secrets.archive"
      }),
    )
    .icon(Icon::Archive);
    let right = if props.archive {
      let (action, open, id, vault) = (
        action.clone(),
        props.project.open.clone(),
        props.secret.id(),
        Services::of(ctx).vault,
      );
      let name = props.secret.name().as_str().to_owned();
      right.on_click(move || {
        if action.state().get().is_pending() {
          return;
        }
        action.run(work::Start {
          repo: open.repo_root.clone(),
          vault: vault.clone(),
          write: work::Write::Archive(id),
          value: None,
          spec: operations::secret_archive(&open, &name),
        })
      })
    } else {
      let (panel, selected, id) = (props.view.panel.clone(), props.view.selected.clone(), props.secret.id());
      right.on_click(move || {
        selected.set(super::state::Selected(Some(id)));
        panel.set(Panel::Archive);
      })
    };
    let row = Row::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
    let row = if props.archive {
      row.child(Spacer::new().flex(1.0))
    } else {
      row
    };
    let row = row.child(if pending { left.disabled() } else { left });
    let row = if props.archive {
      row
    } else {
      row.child(Spacer::new().flex(1.0))
    };
    row.child(if pending { right.loading() } else { right })
  }
}
pub(super) fn frame(header: Element, body: Element, footer: Element) -> Element {
  Column::new()
    .id("secret-inspector")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(header)
    .child(
      ScrollVertical::new(body)
        .id("secret-inspector-scroll")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .child(footer)
    .into()
}
pub(super) fn header(ctx: &mut Ctx, secret: &Secret) -> Element {
  let tile = credential_avatar("secret-inspector-tile", words::glyph(secret.kind()), true);
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SecretsSpacing::Body.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t(words::kind_key(secret.kind()))).variant(SecretsText::InspectorTitle.role()))
        .child(
          Truncated::new(
            "secret-inspector-name",
            secret.name().as_str(),
            SecretsText::InspectorName.role(),
          )
          .mono()
          .render(ctx),
        ),
    )
    .into()
}
fn metadata(ctx: &Ctx, project: &ShownProject, secret: &Secret, show_hint: bool) -> Element {
  let date = match secret.rotated_at() {
    Some(at) => words::date(ctx, at),
    None => ctx.t_args(
      "secrets.neverRotated",
      [("date", words::date(ctx, secret.created_at()))],
    ),
  };
  let value = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(MASK_HEIGHT))
    .align_items(Alignment::Center)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Text::new(&ctx.t("secrets.mask"))
        .variant(SecretsText::Mask.role())
        .flex(1.0),
    )
    .child(icon(Icon::LockKeyhole, IconSize::Base, PaletteColor::TextMuted));
  let value = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("secrets.value")).variant(TypographyStyle::FieldLabel))
    .with_children((!secret.is_archived()).then_some(value))
    .with_children(show_hint.then(|| {
      Text::new(&ctx.t(if secret.is_archived() {
        "secrets.archivedHint"
      } else {
        "secrets.valueHint"
      }))
      .variant(SecretsText::ValueHint.role())
    }));
  let properties = Column::new()
    .width(Dimension::Pct(100.0))
    .child(inspector::property(
      "secret-project",
      &ctx.t("secrets.project"),
      project.open.project.name().as_str(),
      inspector::PropertyValue::Plain,
    ))
    .child(inspector::property(
      "secret-environment",
      &ctx.t("secrets.environment"),
      &ctx.t(words::environment_key(secret.environment())),
      inspector::PropertyValue::Plain,
    ))
    .child(inspector::property(
      "secret-rotated",
      &ctx.t("secrets.rotated"),
      &date,
      inspector::PropertyValue::Plain,
    ));
  let mut body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SecretsSpacing::Body.role())
    .child(value)
    .child(properties);
  if secret.is_archived() {
    body = body.child(Text::new(&ctx.t("secrets.archived")).variant(TypographyStyle::Label));
  }
  body.into()
}
pub(super) fn uses(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectSecrets, secret: &Secret) -> Element {
  let counts = listed.use_counts(secret.id());
  let title = if counts.total() == 0 {
    ctx.t("secrets.uses.noneHeading")
  } else {
    ctx.t_args(
      "secrets.uses.heading",
      [("uses", words::uses(ctx, counts).to_uppercase())],
    )
  };
  let mut section = inspector::section("secret-uses", SpacingSize::Sm)
    .padding(Padding::new().top(SecretsSpacing::Body.role()))
    .child(Text::new(&title).variant(SecretsText::ColumnHeading.role()));
  let navigation = Navigation::of(ctx);
  for used in listed.used_by(secret.id()) {
    let (glyph, label, page) = match used.kind {
      SecretUseKind::Tool(_) => (Icon::Wrench, used.name.clone(), ProjectPage::Tools),
      SecretUseKind::McpServer(id) => (
        Icon::Server,
        ctx.t_args("secrets.uses.mcp", [("name", used.name.clone())]),
        ProjectPage::Connections(ConnectionsView::new(ConnectionsTab::McpServers).with_selected(id)),
      ),
    };
    let (navigation, project_id) = (navigation.clone(), project.open.project.id());
    section = section.child(inspector::used_by_link(
      &format!("secret-use-{:?}", used.kind),
      glyph,
      &label,
      Some(Arc::new(move || {
        navigation.go(Route::Project(project_id, page.clone()))
      })),
    ));
  }
  let key = match (counts.tools, counts.mcp_servers) {
    (0, 0) => "secrets.uses.empty",
    (0, _) => "secrets.uses.serverBindings",
    (_, 0) => "secrets.uses.toolBindings",
    _ => "secrets.uses.bindings",
  };
  section
    .child(inspector::note(
      "secret-use-note",
      Icon::Info,
      &ctx.t_args(key, [("project", project.open.project.name().as_str())]),
    ))
    .into()
}
