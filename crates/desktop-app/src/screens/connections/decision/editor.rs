//! Add and Edit decision model, sharing draft checks and secret handling (01 XMNUP and 01b z3G09h).
mod actions;
mod add;
use std::sync::OnceLock;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{DecisionConnection, DecisionServerUrl};
use orchester_workspace::connections::{data::ProjectConnections, decision::DecisionDraft, native::KeySecret};

use crate::{
  project::ShownProject,
  screens::project_settings::kit::{self, FormField, Group, page_head},
  theme::ExtraSpacing,
  ui::message::message_text,
};

#[derive(Clone)]
pub(in super::super) struct Props {
  pub project: ShownProject,
  pub connection: Option<DecisionConnection>,
  pub listed: ProjectConnections,
  pub close: Signal<bool>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.connection == other.connection && self.listed == other.listed
  }
}
impl DevtoolsInspectable for Props {}

pub(in super::super) struct DecisionEditor {
  fields: super::form::DecisionForm,
  original: OnceLock<DecisionConnection>,
  work: actions::Action,
  refused: Signal<bool>,
  checking: Signal<bool>,
  server: Signal<super::super::add_decision::Server>,
}
impl Component for DecisionEditor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let close = ctx.props::<Props>().close.clone();
    let adding = ctx.props::<Props>().connection.is_none();
    let project = ctx.props::<Props>().project.open.project.id();
    let navigation = crate::navigation::Navigation::of(ctx);
    let work = actions::action(ctx);
    ctx.watch(&work.state(), move |state| {
      if state.is_fulfilled()
        && let Some(actions::Done::Saved(id)) = state.data
      {
        if adding {
          navigation.replace(crate::routes::Route::Project(
            project,
            crate::routes::ProjectPage::Connections(crate::routes::ConnectionsView::decision(id)),
          ));
        } else {
          close.set(false);
        }
      }
    });
    Self {
      fields: super::form::DecisionForm::new(ctx),
      original: OnceLock::new(),
      work,
      refused: ctx.signal(false),
      checking: ctx.signal(false),
      server: ctx.signal(super::super::add_decision::Server::TypeSafe),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    if let Some(connection) = &props.connection {
      self.original.get_or_init(|| {
        self.fields.seed(connection);
        connection.clone()
      });
    } else if self.fields.url.get_untracked().is_empty() && !self.fields.initialized.get_untracked() {
      self.fields.initialized.set(true);
      self.fields.url.set("https://api.typesafe.ai/v1".to_owned());
    }
    let local_preset = props.connection.is_none()
      && self.server.get() == super::super::add_decision::Server::Local
      && DecisionServerUrl::new(&self.fields.url.get()).is_ok_and(|url| url.is_on_this_computer());
    let mut rows = self.fields.rows(ctx, &props.listed, local_preset);
    rows.push(self.models(ctx, &props));
    if props.connection.is_none() {
      return self.add_page(ctx, &props, rows);
    }
    let close = props.close.clone();
    let back = crate::ui::button::ActionButton::new(
      "decision-edit-back",
      crate::ui::button::ButtonKind::Ghost,
      ctx.t("decision_connection.backToConnection"),
    )
    .icon(crate::ui::icon::Icon::ArrowLeft)
    .on_click(move || close.set(false));
    let bar = self.bar(ctx, &props);
    let notice = self.destination(ctx);
    kit::scroll(
      Column::new()
        .id("decision-editor")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Lg)
        .child(back)
        .child(page_head(
          "decision-edit-head",
          &ctx.t("decision_connection.edit"),
          Some(&ctx.t("decision_connection.editLead")),
        ))
        .child(Group::new("decision-edit-fields", ctx.t("decision_connection.connection")).rows(rows))
        .child(notice)
        .child(bar),
    )
  }
}

impl DecisionEditor {
  fn draft(&self, props: &Props) -> Option<(DecisionDraft, Option<KeySecret>)> {
    self
      .fields
      .draft(props.connection.as_ref().map(DecisionConnection::id), &props.listed)
  }

  fn models(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let state = self.work.state().get();
    let preview = match state.data.as_ref() {
      Some(actions::Done::Checked(checked))
        if checked.revision == self.fields.key.key_revision()
          && self.draft(props).as_ref() == Some(&(checked.draft.clone(), checked.key.clone())) =>
      {
        Some(&checked.connection)
      }
      _ => None,
    };
    let unchanged = props.connection.as_ref().filter(|connection| {
      self.draft(props).is_some_and(|(draft, key)| {
        draft.base_url == *connection.base_url() && key == connection.key().map(KeySecret::Existing)
      })
    });
    let shown = preview.or(unchanged);
    let pills = shown
      .map(|connection| {
        connection
          .models()
          .iter()
          .enumerate()
          .map(|(index, model)| {
            Element::from(
              crate::ui::chip::TagChip::new(format!("decision-edit-model-{index}"), model.as_str())
                .mono()
                .pill(),
            )
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    let row = Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role())
      .with_children(pills);
    let check = self.check_button(ctx, props);
    let status = shown.map(|connection| super::words::line(ctx, connection, state.is_pending() && self.checking.get()));
    FormField::new(
      "decision-edit-models",
      ctx.t(if props.connection.is_none() {
        "decision_connection.model"
      } else {
        "decision_connection.models"
      }),
    )
    .hint(ctx.t("decision_connection.modelsHint"))
    .render(
      ctx,
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .align_items(lurq::layout::Alignment::Center)
            .child(row)
            .child(check),
        )
        .with_children(status),
    )
  }

  fn destination(&self, ctx: &mut Ctx) -> Element {
    let url = DecisionServerUrl::new(&self.fields.url.get());
    let local = url.as_ref().is_ok_and(|url| url.is_on_this_computer());
    let text = if local {
      ctx.t(if ctx.props::<Props>().connection.is_none() {
        "decision_connection.addLocal"
      } else {
        "decision_connection.editLocal"
      })
    } else {
      let host = url.map_or_else(
        |_| ctx.t("decision_connection.server").to_string(),
        |url| url.host().to_owned(),
      );
      ctx.t_args("decision_connection.editHosted", [("host", host.as_str())])
    };
    crate::screens::project_settings::kit::notice_line(
      ctx,
      (
        "decision-edit-destination",
        if local {
          crate::ui::icon::Icon::Monitor
        } else {
          crate::ui::icon::Icon::Send
        },
      ),
      &text,
      None,
      if local {
        crate::ui::notice::NoticeTone::Success
      } else {
        crate::ui::notice::NoticeTone::Warning
      },
    )
  }

  fn error(&self, ctx: &Ctx) -> Option<String> {
    self.work.state().get().error.map(|e| message_text(ctx, &e).to_string())
  }
}
