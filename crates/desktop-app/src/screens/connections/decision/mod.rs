//! Decision model connection page: frame 02 GM4yE and frame 07 FJRFV.
pub(super) mod editor;
pub(super) mod form;
mod groups;
mod read;
mod test;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::DecisionConnection, id::DecisionConnectionId};
use orchester_workspace::{messages::Message, tasks::outside::Outside};
use read::Read;
use work::Work;

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  screens::project_settings::kit::{self, Group, SettingRow, back, entity_head, tile},
  theme::ProjectSettingsText,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::Dialog,
    field::field_error,
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel, separator},
    message::message_text,
    state::load_error,
  },
};

/// The connection menu in 02 (`Zf5V1`).
const MENU_WIDTH: f32 = 220.0;

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub connection: DecisionConnectionId,
}
impl DevtoolsInspectable for Props {}

pub(super) struct DecisionPage {
  action: work::Action,
  revision: Signal<u64>,
  more: (Signal<bool>, ElementRef),
  deleting: Signal<bool>,
  checking: Signal<bool>,
  editing: Signal<bool>,
  read_failed: Signal<bool>,
}

impl Component for DecisionPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let action = work::action(ctx);
    let revision = ctx.signal(0);
    let reread = revision.clone();
    let navigation = Navigation::of(ctx);
    let project = ctx.props::<Props>().project.open.project.id();
    ctx.watch(&action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      reread.update(|value| *value += 1);
      if state.is_fulfilled() && state.data == Some(true) {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ));
      }
    });
    Self {
      action,
      revision,
      more: (ctx.signal(false), ctx.element_ref()),
      deleting: ctx.signal(false),
      checking: ctx.signal(false),
      editing: ctx.signal(false),
      read_failed: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let outside = props.project.feed.outside.get();
    let revision = self.revision.get() + outside.of(Outside::Connections) + outside.of(Outside::AutoMode);
    let loaded = read::load(ctx, props.project.open.repo_root.clone(), props.connection, revision);
    if let Some(error) = &loaded.error {
      if !self.read_failed.get_untracked() {
        self.read_failed.set(true);
      }
      return kit::scroll(self.failed(ctx, error));
    }
    if self.read_failed.get() {
      if !loaded.is_fulfilled() {
        return kit::scroll(Text::new(&ctx.t("connections.reading")));
      }
      self.read_failed.set(false);
    }
    let Some(read) = loaded.data else {
      return kit::scroll(Text::new(&ctx.t("connections.reading")));
    };
    let Some(connection) = &read.connection else {
      let open = Navigation::of(ctx).to(Route::Project(
        props.project.open.project.id(),
        ProjectPage::Connections(ConnectionsView::default()),
      ));
      return kit::scroll(
        Column::new()
          .child(back(
            "decision-back",
            (&ctx.t("project_settings.connections"), None),
            open,
          ))
          .child(Text::new(&ctx.t("decision_connection.gone"))),
      );
    };
    if self.editing.get() {
      return ctx.mount_keyed::<editor::DecisionEditor>(
        &format!("decision-editor-{}", connection.id()),
        editor::Props {
          project: props.project.clone(),
          connection: Some(connection.clone()),
          listed: read.listed.clone(),
          close: self.editing.clone(),
        },
      );
    }
    let content = self.page(ctx, &props, &read, connection);
    let dialog = self.delete_dialog(ctx, &props, &read, connection);
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(kit::scroll(content))
      .child(dialog)
      .into()
  }
}

impl DecisionPage {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.revision.clone();
    load_error(
      ctx,
      "decision-load-error",
      (&ctx.t("connections.loadFailed"), &message_text(ctx, error)),
      move || revision.update(|value| *value += 1),
    )
  }

  fn page(&self, ctx: &mut Ctx, props: &Props, read: &Read, connection: &DecisionConnection) -> Element {
    let open = Navigation::of(ctx).to(Route::Project(
      props.project.open.project.id(),
      ProjectPage::Connections(ConnectionsView::default()),
    ));
    let pending = self.checking.get() && self.action.state().get().is_pending();
    let status = words::line(ctx, connection, pending);
    let title = Text::new(connection.name().as_str())
      .id("decision-title")
      .variant(ProjectSettingsText::PageTitle.role());
    let check = self.header_action(
      ctx,
      props,
      read,
      connection,
      ("decision-check", "decision_connection.checkAgain"),
    );
    let menu = self.menu(ctx, read);
    let head = entity_head(
      ctx,
      "decision-header",
      (glyph(connection).into(), title.into()),
      status,
      vec![check.into(), menu],
    );
    let error = self
      .action
      .state()
      .get()
      .error
      .map(|error| field_error("decision-work-error", &message_text(ctx, &error)));
    let note = words::note_key(connection).filter(|_| !pending).map(|key| {
      Text::new(&ctx.t(key))
        .id("decision-note")
        .variant(ProjectSettingsText::Hint.role())
        .width(Dimension::Pct(100.0))
    });
    let server = groups::server(ctx, read, connection);
    let models = groups::models(ctx, self, props, read, connection);
    let used = groups::used_by(ctx, props, read, connection);
    let test = ctx.mount_keyed::<test::TestSection>(
      &format!("decision-test-{}", connection.id()),
      test::Props {
        project: props.project.clone(),
        connection: connection.clone(),
        reviewer: read.reviewer.clone(),
      },
    );
    Column::new()
      .id("decision-connection-page")
      .describe("decision-connection", connection.id().to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(back(
        "decision-back",
        (&ctx.t("project_settings.connections"), None),
        open,
      ))
      .child(head)
      .with_children(note)
      .with_children(error)
      .child(server)
      .child(models)
      .child(test)
      .child(used)
      .into()
  }

  fn header_action(
    &self,
    ctx: &Ctx,
    props: &Props,
    read: &Read,
    connection: &DecisionConnection,
    words: (&str, &str),
  ) -> ActionButton {
    use orchester_core::connection::DecisionCheckResult;
    if connection.last_check().is_some_and(|check| {
      matches!(
        check.result,
        DecisionCheckResult::KeyRefused | DecisionCheckResult::NotSystemOneApi
      )
    }) {
      let editing = self.editing.clone();
      return ActionButton::new(
        "decision-repair",
        ButtonKind::Secondary,
        ctx.t("decision_connection.edit"),
      )
      .icon(Icon::Settings)
      .on_click(move || editing.set(true));
    }
    self.check_button(ctx, props, read, connection, words)
  }

  fn check_button(
    &self,
    ctx: &Ctx,
    props: &Props,
    read: &Read,
    connection: &DecisionConnection,
    (id, key): (&str, &str),
  ) -> ActionButton {
    let action = self.action.clone();
    let input = (
      props.project.open.clone(),
      connection.id(),
      connection.name().as_str().to_owned(),
      Work::Check(read.reviewer.as_ref().map(|choice| choice.model.clone())),
    );
    let kind = if id == "decision-refresh" {
      ButtonKind::Ghost
    } else {
      ButtonKind::Secondary
    };
    let checking = self.checking.clone();
    let button = ActionButton::new(id.to_owned(), kind, ctx.t(key))
      .icon(Icon::RefreshCw)
      .on_click(move || {
        checking.set(true);
        action.run(input.clone());
      });
    if !self.action.state().get().is_pending() {
      button
    } else if self.checking.get() {
      button.loading()
    } else {
      button.disabled()
    }
  }

  fn menu(&self, ctx: &mut Ctx, read: &Read) -> Element {
    let (open, anchor) = self.more.clone();
    let toggle = open.clone();
    let button = IconButton::new("decision-more", Icon::Ellipsis, ctx.t("connections.more"))
      .element(anchor.clone())
      .on_click(move || toggle.update(|value| *value = !*value));
    let button = if open.get() { button.active() } else { button }.render(ctx);
    let editing = self.editing.clone();
    let edit = MenuItem::new("decision-edit", ctx.t("decision_connection.edit"), move || {
      editing.set(true)
    })
    .icon(Icon::Settings);
    let delete = if read.reviewer.is_some() || self.action.state().get().is_pending() {
      MenuItem::unavailable(
        "decision-delete",
        ctx.t("decision_connection.delete"),
        ctx.t("decision_connection.deleteUsed"),
      )
    } else {
      let deleting = self.deleting.clone();
      MenuItem::new("decision-delete", ctx.t("decision_connection.delete"), move || {
        deleting.set(true)
      })
    }
    .danger()
    .icon(Icon::Trash2);
    let panel = menu_panel("decision-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(edit.render(ctx, &open))
      .child(separator(ctx))
      .child(delete.render(ctx, &open));
    Row::new().child(button).child(menu_at_end(anchor, open, panel)).into()
  }

  fn delete_dialog(&self, ctx: &Ctx, props: &Props, read: &Read, connection: &DecisionConnection) -> Element {
    let action = self.action.clone();
    let close = self.deleting.clone();
    let checking = self.checking.clone();
    let input = (
      props.project.open.clone(),
      connection.id(),
      connection.name().as_str().to_owned(),
      Work::Delete,
    );
    let delete = ActionButton::new(
      "decision-delete-confirm",
      ButtonKind::Danger,
      ctx.t("decision_connection.delete"),
    )
    .on_click(move || {
      close.set(false);
      checking.set(false);
      action.run(input.clone());
    });
    let delete = if read.reviewer.is_some() || self.action.state().get().is_pending() {
      delete.disabled()
    } else {
      delete
    };
    Dialog::new(
      "decision-delete-dialog",
      Icon::Trash2,
      ctx.t("decision_connection.deleteTitle"),
      ctx.t("decision_connection.cancel"),
    )
    .part(Text::new(&ctx.t("decision_connection.deleteBody")).variant(ProjectSettingsText::Hint.role()))
    .action(delete)
    .render(ctx, &self.deleting)
  }
}

fn glyph(connection: &DecisionConnection) -> Icon {
  if connection.is_on_this_computer() {
    Icon::Monitor
  } else {
    Icon::Scale
  }
}

/// Decision servers and the entry to Add decision model.
pub(super) fn listed(ctx: &mut Ctx, project: &ShownProject, connections: &[DecisionConnection]) -> Element {
  let navigation = Navigation::of(ctx);
  let rows = connections
    .iter()
    .map(|connection| {
      let open = navigation.to(Route::Project(
        project.open.project.id(),
        ProjectPage::Connections(ConnectionsView::decision(connection.id())),
      ));
      SettingRow::new(format!("decision-row-{}", connection.id()), connection.name().as_str())
        .item()
        .lead(tile(ctx, glyph(connection)))
        .hint(connection.base_url().as_str())
        .control(crate::ui::icon::icon(
          Icon::ChevronRight,
          crate::theme::IconSize::Md,
          lurq::app::theme::PaletteColor::TextMuted,
        ))
        .opens(open)
        .render(ctx)
    })
    .collect::<Vec<_>>();
  Group::new("connections-decisions", ctx.t("decision_connection.group"))
    .end(
      ActionButton::new(
        "connections-add-decision",
        ButtonKind::Ghost,
        ctx.t("decision_connection.add"),
      )
      .icon(Icon::Plus)
      .compact()
      .opens(navigation.to(Route::Project(
        project.open.project.id(),
        ProjectPage::NewConnection(crate::routes::NewConnection::DecisionModel),
      ))),
    )
    .rows(rows)
    .into()
}
