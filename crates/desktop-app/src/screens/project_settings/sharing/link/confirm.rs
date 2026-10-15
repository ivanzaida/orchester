//! *Link to Kontur…* and its questions in place (`N9NFio`): the mismatch warning with *Choose another project* and
//! *Link anyway…*, then what goes up with *Link and upload*, and why a link did not go ahead.

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
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::Said,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Label, Place, side_row},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

impl Link {
  /// *Link to Kontur…* and, once pressed, the questions in place and what came of them.
  pub(super) fn link_row(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let resolving = self.resolve.state().get();
    let linking = self.link.state().get();
    let target = resolving
      .data
      .clone()
      .filter(|_| resolving.is_fulfilled() && self.asking.get() != Asking::Nothing);
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm);
    control = match (self.asking.get(), &target) {
      (Asking::Mismatch, Some(target)) => control.child(self.mismatch(ctx, target)),
      (Asking::Upload, Some(target)) => control.child(self.upload(ctx, target, props, linking.is_pending())),
      _ => control.child(self.link_button(ctx, self.choice(props, listed), resolving.is_pending())),
    };
    let problem = resolving
      .error
      .clone()
      .filter(|_| !resolving.is_pending())
      .or_else(|| linking.error.clone().filter(|_| !linking.is_pending()));
    if let Some(problem) = problem {
      control = control.child(problem_part(ctx, &problem));
    }
    control = control.child(page::icon_note(
      "sharing-link-hint",
      Icon::Info,
      &ctx.t("sharing.link.hint"),
    ));
    side_row(
      Label::new("sharing-link", ctx.t("sharing.link.link")),
      control,
      Place::Last,
      true,
    )
  }

  /// What the person chose: an address typed or taken, else the project picked in the list.
  fn choice(&self, props: &Props, listed: Option<&Result<Listed, Said>>) -> Option<Choice> {
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
    let button = ActionButton::new("sharing-link-button", ButtonKind::Secondary, label).icon(Icon::Link);
    let button = match choice {
      _ if pending => button.loading(),
      Some(choice) => {
        let resolve = self.resolve.clone();
        button.on_click(move || resolve.run(choice.clone()))
      }
      None => button.unavailable(ctx.t("sharing.link.chooseFirst")),
    };
    Row::new().child(button).into()
  }

  /// *This checkout's origin is github.com/acme/kontur* with *Choose another project* and *Link anyway…*.
  fn mismatch(&self, ctx: &Ctx, target: &Target) -> Element {
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
    let asking = self.asking.clone();
    let anyway = ActionButton::new(
      "sharing-link-anyway",
      ButtonKind::Ghost,
      ctx.t("sharing.link.mismatch.anyway"),
    )
    .on_click(move || asking.set(Asking::Upload));
    let body = ctx.t_args(
      "sharing.link.mismatch.body",
      [
        ("project", target.0.project.name.as_str()),
        ("server", target.0.entry.stored.name.as_str()),
        ("hint", hint.as_str()),
      ],
    );
    Notice::new(
      "sharing-mismatch",
      NoticeTone::Warning,
      ctx.t_args("sharing.link.mismatch.title", [("origin", checkout.as_str())]),
    )
    .icon(Icon::TriangleAlert)
    .body(body)
    .action(another)
    .action(anyway)
    .stacked()
    .into()
  }

  /// What goes up, with *Link and upload N tasks* and *Cancel*.
  fn upload(&self, ctx: &Ctx, target: &Target, props: &Props, pending: bool) -> Element {
    let count = props.facts.tasks.to_string();
    let body = ctx.t_args(
      "sharing.link.upload.body",
      [
        ("count", count.clone()),
        ("key", props.project.open.project.key().as_str().to_owned()),
        ("address", target.0.project.address.clone()),
      ],
    );
    let (link, chosen, key) = (self.link.clone(), target.clone(), self.key.get());
    let confirm = ActionButton::new(
      "sharing-link-confirm",
      ButtonKind::Primary,
      ctx.t_args("sharing.link.upload.confirm", [("count", count)]),
    )
    .icon(Icon::Upload);
    let confirm = if pending {
      confirm.loading()
    } else {
      confirm.on_click(move || link.run((chosen.clone(), key.clone())))
    };
    let asking = self.asking.clone();
    let cancel = ActionButton::new(
      "sharing-link-cancel",
      ButtonKind::Ghost,
      ctx.t("sharing.link.upload.cancel"),
    )
    .on_click(move || asking.set(Asking::Nothing));
    Notice::new(
      "sharing-upload",
      NoticeTone::Info,
      ctx.t_args(
        "sharing.link.upload.title",
        [("project", target.0.project.name.as_str())],
      ),
    )
    .icon(Icon::Link)
    .body(body)
    .action(confirm)
    .action(cancel)
    .stacked()
    .into()
  }
}

fn problem_part(ctx: &mut Ctx, problem: &Problem) -> Element {
  match problem {
    Problem::NotConnected(host) => {
      let navigation = Navigation::of(ctx);
      Notice::new(
        "sharing-not-connected",
        NoticeTone::Info,
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
        .on_click(move || navigation.go(Route::Orchester(OrchesterView::connect()))),
      )
      .stacked()
      .into()
    }
    Problem::NoProject(address) => field_error(
      "sharing-no-project",
      &ctx.t_args("sharing.link.noProject", [("address", address.as_str())]),
    )
    .into(),
    Problem::Taken(project) => Notice::new(
      "sharing-taken",
      NoticeTone::Warning,
      ctx.t_args("sharing.link.taken.title", [("project", project.as_str())]),
    )
    .body(ctx.t("sharing.link.taken.body"))
    .stacked()
    .into(),
    Problem::Other(said) => field_error("sharing-link-error", &said.text(ctx)).into(),
  }
}
