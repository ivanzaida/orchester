//! The widens bar with *Use v<N> here* (`V097I` in 02d), which only the person presses.
use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, resource::VersionNumber, run::Answerer};
use orchester_workspace::{background, sharing::sync};

use super::{Waiting, adds::adds_list};
use crate::{
  screens::{
    hub::draft_bar::{DraftBar, Standing as BarStanding},
    settings::orchester::{PersonOnly, Said, sharing_said},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
  },
};

/// *Use v<N> here*, which only a person presses.
const USE_HERE: &str = "agent-use-here";

#[derive(Clone)]
pub(super) struct UseHereProps {
  pub root: PathBuf,
  pub resource: ResourceId,
  pub waiting: Waiting,
  pub revision: Signal<u64>,
}

impl PartialEq for UseHereProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.resource == other.resource && self.waiting == other.waiting
  }
}

impl DevtoolsInspectable for UseHereProps {}

/// The widens bar with *Use v<N> here* (`V097I` in 02d).
pub(super) struct UseHere {
  /// Takes the version the bar shows when pressed, so a newer version that arrives while it is mounted is the one
  /// used.
  use_here: FutureAction<VersionNumber, (), Said>,
  person: PersonOnly,
}

impl Component for UseHere {
  type Props = UseHereProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<UseHereProps>().clone();
    let use_here = ctx.future_action(move |number: VersionNumber| {
      let (root, resource, revision) = (props.root.clone(), props.resource, props.revision.clone());
      async move {
        background::run(move || sync::use_here(&root, resource, number, Answerer::Person))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_said(&error))?;
        revision.update(|count| *count += 1);
        Ok(())
      }
    });
    Self {
      use_here,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UseHereProps>().clone();
    let version = props.waiting.number.get().to_string();
    let adds = adds_list(ctx, &props.waiting);
    let state = self.use_here.state().get();
    let button = ActionButton::new(
      USE_HERE,
      ButtonKind::Primary,
      ctx.t_args("sharing.config.widens.use", [("version", version.as_str())]),
    )
    .icon(Icon::Check);
    let button = if state.is_pending() {
      button.loading()
    } else {
      let (action, person, number) = (self.use_here.clone(), self.person.clone(), props.waiting.number);
      button.on_click(move || {
        if person.allows(USE_HERE) {
          action.run(number);
        }
      })
    };
    let names = |used: &str| {
      [
        ("publisher", props.waiting.publisher.clone()),
        ("version", version.clone()),
        ("used", used.to_owned()),
      ]
    };
    let (sub, compare) = match props.waiting.used {
      Some(used) => {
        let used = used.get().to_string();
        // Profiles have no compare view yet: the action is shown, unavailable, with why.
        let compare = ActionButton::new(
          "agent-widens-compare",
          ButtonKind::Secondary,
          ctx.t_args("sharing.config.widens.compare", names(&used)),
        )
        .unavailable(ctx.t("sharing.config.widens.compareLater"));
        (
          ctx.t_args("sharing.config.widens.bodyUsed", names(&used)),
          Some(compare),
        )
      }
      None => (ctx.t_args("sharing.config.widens.body", names("")), None),
    };
    let mut bar = DraftBar::new(
      "agent-widens-bar",
      BarStanding::Widens,
      ctx.t_args("sharing.config.widens.title", [("version", version.as_str())]),
    )
    .sub(sub)
    .list(adds)
    .actions_below()
    .foot(ctx.t("sharing.config.widens.personOnly"));
    if let Some(compare) = compare {
      bar = bar.action(compare);
    }
    let bar = bar.action(button);
    let failed = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("agent-use-here-error", &reason.text(ctx)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(bar)
      .with_children(self.person.reason(ctx, USE_HERE))
      .with_children(failed)
  }
}
