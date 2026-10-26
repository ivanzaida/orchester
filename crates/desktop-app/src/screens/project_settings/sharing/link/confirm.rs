//! *Link to Kontur…* and its question in place (`N9NFio`): the mismatch warning with *Choose another project* and
//! *Link anyway…*, which opens what goes up on its page (`KSnOW`) or the join review (`AiCwO`), and why a link did not
//! go ahead: *No project you're a member of here* with *Ask for an invite* (`oOGMT`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::sharing::RemoteMatch;

use super::{Asking, Choice, Link, Listed, Picked, Problem, Props, Target};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  screens::{project_settings::kit::form_actions, settings::orchester::Said},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::Icon,
  },
};

/// *Link to Kontur…*, which only a person presses.
const LINK: &str = "sharing-link-button";

impl Link {
  /// *Cancel* and *Link to Kontur…* at the page's end (`Actions` in `JeXKt`) and, once pressed, the questions in place
  /// and what came of them.
  pub(super) fn link_row(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let resolving = self.resolve.state().get();
    let target = resolving
      .data
      .clone()
      .filter(|_| resolving.is_fulfilled() && self.asking.get() != Asking::Nothing);
    let mut column = Column::new()
      .id("sharing-link")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm);
    column = match (self.asking.get(), &target) {
      (Asking::Mismatch, Some(target)) => column.child(self.mismatch(ctx, target, props)),
      _ => {
        let sharing = self.sharing.clone();
        let cancel = ActionButton::new("sharing-share-cancel", ButtonKind::Ghost, ctx.t("sharing.share.cancel"))
          .on_click(move || sharing.set(false));
        let link = self.link_button(ctx, self.choice(props, listed), resolving.is_pending());
        let hint = ctx.t("sharing.link.hint");
        column.child(form_actions(
          "sharing-link-actions",
          vec![cancel.into(), link],
          Some(&hint),
        ))
      }
    };
    column = column.with_children(self.person.reason(ctx, LINK));
    let problem = resolving.error.clone().filter(|_| !resolving.is_pending());
    if let Some(problem) = problem {
      column = column.with_children(problem_part(ctx, &problem));
    }
    column.into()
  }

  /// What the person chose: an address typed or taken, else the project picked in the list.
  pub(super) fn choice(&self, props: &Props, listed: Option<&Result<Listed, Said>>) -> Option<Choice> {
    let typed = self.address.get();
    if !typed.trim().is_empty() {
      return Some(Choice::Address(typed));
    }
    let project = self.picked_project(listed)?.clone();
    Some(Choice::Listed(Box::new((self.chosen_entry(props)?.clone(), project))))
  }

  fn link_button(&self, ctx: &Ctx, choice: Option<Choice>, pending: bool) -> Element {
    let label = match &choice {
      Some(Choice::Listed(listed)) => ctx.t_args("sharing.link.linkTo", [("project", listed.1.name.as_str())]),
      _ => ctx.t("sharing.link.linkAddress"),
    };
    let button = ActionButton::new(LINK, ButtonKind::Primary, label).icon(Icon::Link);
    let button = match choice {
      _ if pending => button.loading(),
      Some(choice) => {
        let (resolve, person) = (self.resolve.clone(), self.person.clone());
        button.on_click(move || {
          if person.allows(LINK) {
            resolve.run(choice.clone());
          }
        })
      }
      None => button.unavailable(ctx.t("sharing.link.chooseFirst")),
    };
    Row::new().child(button).into()
  }

  /// *This checkout's origin is github.com/acme/kontur* with *Choose another project* and *Link anyway…*.
  fn mismatch(&self, ctx: &mut Ctx, target: &Target, props: &Props) -> Element {
    let RemoteMatch::Different { checkout, hint } = &target.0.remote else {
      return Column::new().into();
    };
    let (asking, picked) = (self.asking.clone(), self.picked.clone());
    let another = ActionButton::new(
      "sharing-choose-another",
      ButtonKind::Secondary,
      ctx.t("sharing.link.mismatch.another"),
    )
    .on_click(move || {
      asking.set(Asking::Nothing);
      picked.set(Picked(None));
    });
    let (asking, page, chosen) = (self.asking.clone(), props.page.clone(), target.clone());
    let (navigation, project) = (Navigation::of(ctx), props.project.open.project.id());
    let anyway = ActionButton::new(
      "sharing-link-anyway",
      ButtonKind::Ghost,
      ctx.t("sharing.link.mismatch.anyway"),
    )
    .on_click(move || {
      asking.set(Asking::Nothing);
      chosen.open(&page, (&navigation, project));
    });
    let body = ctx.t_args(
      "sharing.link.mismatch.body",
      [
        ("project", target.0.project.name.as_str()),
        ("server", target.0.entry.stored.name.as_str()),
        ("hint", hint.as_str()),
      ],
    );
    Callout::new(
      "sharing-mismatch",
      CalloutTone::Warning,
      ctx.t_args("sharing.link.mismatch.title", [("origin", checkout.as_str())]),
    )
    .icon(Icon::TriangleAlert)
    .body(body)
    .action(another)
    .action(anyway)
    .into()
  }
}

fn problem_part(ctx: &mut Ctx, problem: &Problem) -> Option<Element> {
  Some(match problem {
    Problem::NotConnected(host) => {
      let navigation = Navigation::of(ctx);
      Callout::new(
        "sharing-not-connected",
        CalloutTone::Info,
        ctx.t_args("sharing.link.notConnected.title", [("host", host.as_str())]),
      )
      .icon(Icon::Server)
      .body(ctx.t_args("sharing.link.notConnected.body", [("host", host.as_str())]))
      .action(
        ActionButton::new(
          "sharing-not-connected-connect",
          ButtonKind::Secondary,
          ctx.t("settings.connectServer"),
        )
        .icon(Icon::Plus)
        .opens(navigation.to(Route::Orchester(OrchesterView::connect()))),
      )
      .into()
    }
    // Said at the section's width, under it ([`Link::not_member`]).
    Problem::NoProject(..) => return None,
    Problem::Other(said) => field_error("sharing-link-error", &said.text(ctx)).into(),
  })
}
