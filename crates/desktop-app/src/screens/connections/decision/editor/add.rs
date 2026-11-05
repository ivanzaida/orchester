//! Add's server presets and page frame (XMNUP / z3G09h).
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};

use super::{DecisionEditor, Props};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  screens::{
    connections::{
      add_decision::Server,
      page::{self, Head},
    },
    project_settings::kit::Group,
  },
  ui::{icon::Icon, option_card::OptionCard},
};

impl DecisionEditor {
  pub(super) fn add_page(&self, ctx: &mut Ctx, props: &Props, rows: Vec<Element>) -> Element {
    let servers = self.servers(ctx);
    let notice = self.destination(ctx);
    let bar = self.bar(ctx, props);
    let head = Head {
      back: Navigation::of(ctx).to(Route::Project(
        props.project.open.project.id(),
        ProjectPage::Connections(ConnectionsView::default()),
      )),
      here: ctx.t("decision_connection.addCrumb"),
      title: ctx.t("decision_connection.addTitle"),
      description: Some(ctx.t("decision_connection.addLead")),
    };
    page::page(
      ctx,
      Screen::NewConnection,
      head,
      vec![
        servers,
        Group::new("decision-add-fields", ctx.t("decision_connection.connection"))
          .rows(rows)
          .into(),
        notice,
      ],
      bar,
    )
  }

  fn servers(&self, ctx: &Ctx) -> Element {
    let cards = [
      (
        Server::TypeSafe,
        "typesafe",
        Icon::Cloud,
        "decision_connection.typeSafe",
        "decision_connection.typeSafeHint",
        "https://api.typesafe.ai/v1",
      ),
      (
        Server::Laya,
        "laya",
        Icon::Cloud,
        "decision_connection.laya",
        "decision_connection.layaHint",
        "https://api.laya.studio/v1",
      ),
      (
        Server::Local,
        "local",
        Icon::Monitor,
        "decision_connection.onComputer",
        "decision_connection.onComputerHint",
        "http://127.0.0.1:8090/v1",
      ),
      (
        Server::Other,
        "other",
        Icon::Server,
        "decision_connection.other",
        "decision_connection.otherHint",
        "",
      ),
    ];
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(cards.into_iter().map(|(choice, id, icon, title, hint, url)| {
        let (server, address) = (self.server.clone(), self.fields.url.clone());
        let card = OptionCard::new(
          format!("decision-server-{id}"),
          (icon, ctx.t(title)),
          ctx.t(hint),
          self.server.get() == choice,
          move || {
            server.set(choice);
            address.set(url.to_owned());
          },
        )
        .compact();
        Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card)
      }));
    Group::new("decision-server-choices", ctx.t("decision_connection.server"))
      .after(row)
      .into()
  }
}
