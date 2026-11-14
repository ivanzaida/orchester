//! A local module's own branch on its way to its base branch, the first row of its Summary (`M02 · Module · Summary ·
//! its branch and Deliver` and `Parts · Delivery` a–j on the board `Orchestrator autonomy · frames to approve`;
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`). It is drawn as the strip a team module's delivery line is
//! (`crate::team::strip`, `04f` `Wd4G1`): the branch and where it goes in Geist Mono with how it stands in a badge,
//! what that rests on under
//! it, and **Deliver** at its end when there is merged work the base branch has not. Deliver asks first, in a dialog:
//! the merge into the base branch is not undone. A delivery git refuses (a conflict, the person's own changes in the
//! way, a run still at work) says why under the row and leaves everything as it was. A module without a branch of
//! its own has no such row.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ModuleId,
  module::delivery::{DeliveryStep, PersonsDelivery},
};
use orchester_workspace::{
  background,
  modules::{
    data::ModulePage,
    delivery::{DeliveryView, deliver_in},
  },
};

use super::save::Refused;
use crate::{
  project::ShownProject,
  screens::settings::orchester::moment,
  services::Services,
  team::strip::{Strip, StripTone},
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    dialog::{Dialog, Tone},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

const ID: &str = "module-delivery";
/// **Deliver**, which a person presses or the conversation that leads the module asks for with its own tool.
pub const DELIVER: &str = "module-deliver";

#[derive(Clone)]
pub struct DeliveryProps {
  pub project: ShownProject,
  pub page: Arc<ModulePage>,
}

impl PartialEq for DeliveryProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && Arc::ptr_eq(&self.page, &other.page)
  }
}

impl DevtoolsInspectable for DeliveryProps {}

/// A delivery that merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivered;

impl DevtoolsInspectable for Delivered {}

type DeliverAction = FutureAction<(PathBuf, ModuleId), Delivered, Refused>;

/// How the module's branch stands, as its row says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stands {
  /// No task of the module was merged into it yet.
  Nothing,
  /// A run of the module has not ended.
  AtWork,
  /// The branch, as it is, is on its base branch.
  Delivered,
  /// Work is merged and tasks are left.
  Open,
  /// Every task is done; its acceptance has not run.
  Accept,
  /// Every task is done; its acceptance did not pass, and one more starts by itself.
  NotPassed,
  Accepting,
  Accepted,
  /// Every task is done and the module names no acceptance pipeline.
  NoAcceptance,
  /// Its acceptances did not pass, as many as are started by themselves.
  Ceiling,
}

impl Stands {
  fn of(view: &DeliveryView) -> Self {
    if view.merged == 0 {
      return Self::Nothing;
    }
    // The acceptance's own run is at work while it accepts.
    if view.step == DeliveryStep::Accepting {
      return Self::Accepting;
    }
    if !view.at_work.is_empty() {
      return Self::AtWork;
    }
    match view.step {
      DeliveryStep::Delivered => Self::Delivered,
      _ if view.open > 0 => Self::Open,
      DeliveryStep::Accept if !view.acceptances.is_empty() => Self::NotPassed,
      DeliveryStep::Accept => Self::Accept,
      DeliveryStep::Accepting => Self::Accepting,
      DeliveryStep::Deliver | DeliveryStep::Person(PersonsDelivery::NotAllowed) => Self::Accepted,
      DeliveryStep::Person(PersonsDelivery::NoAcceptance) => Self::NoAcceptance,
      DeliveryStep::Person(PersonsDelivery::AcceptanceCeiling) => Self::Ceiling,
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Nothing => "nothing",
      Self::AtWork => "at_work",
      Self::Delivered => "delivered",
      Self::Open => "open",
      Self::Accept => "accept",
      Self::NotPassed => "not_passed",
      Self::Accepting => "accepting",
      Self::Accepted => "accepted",
      Self::NoAcceptance => "no_acceptance",
      Self::Ceiling => "ceiling",
    }
  }

  /// The row's tone and its glyph's color.
  const fn tone(self) -> (StripTone, PaletteColor) {
    match self {
      Self::Accepted => (StripTone::Success, PaletteColor::Success),
      Self::NotPassed | Self::Ceiling => (StripTone::Warning, PaletteColor::Warning),
      _ => (StripTone::Plain, PaletteColor::TextMuted),
    }
  }

  /// Whether Deliver shows, and as the row's primary action.
  const fn offers(self) -> Option<ButtonKind> {
    match self {
      Self::Accepted | Self::NoAcceptance => Some(ButtonKind::Primary),
      Self::AtWork | Self::Open | Self::Accept | Self::NotPassed | Self::Ceiling => Some(ButtonKind::Secondary),
      Self::Nothing | Self::Delivered | Self::Accepting => None,
    }
  }
}

/// Whether **Deliver** is the page's primary action for `view`: the header's own then steps back.
pub fn is_primary(view: &DeliveryView) -> bool {
  Stands::of(view).offers() == Some(ButtonKind::Primary)
}

pub struct ModuleDelivery {
  /// The dialog that asks before the merge is open.
  asking: Signal<bool>,
  deliver: DeliverAction,
}

impl Component for ModuleDelivery {
  type Props = DeliveryProps;

  fn create(ctx: &mut Ctx) -> Self {
    let programs = Services::of(ctx).programs;
    let deliver: DeliverAction = ctx.future_action(move |(root, module): (PathBuf, ModuleId)| {
      let programs = programs.clone();
      async move {
        background::run(move || deliver_in(&root, &programs, module))
          .await
          .map_err(Refused)?
          .map(|_| Delivered)
          .map_err(|refusal| {
            tracing::warn!(code = refusal.code(), "the module was not delivered");
            Refused(refusal.message())
          })
      }
    });
    let asking = ctx.signal(false);
    let feed = ctx.props::<DeliveryProps>().project.feed.clone();
    let closes = asking.clone();
    ctx.watch(&deliver.state(), move |state| {
      if state.is_pending() {
        return;
      }
      closes.set(false);
      if state.is_fulfilled() {
        feed.saved();
      }
    });
    Self { asking, deliver }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeliveryProps>().clone();
    let column = Column::new()
      .id("module-delivery-row")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm);
    let Some(view) = &props.page.delivery else {
      return column;
    };
    let stands = Stands::of(view);
    let state = self.deliver.state().get();
    let refused = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|Refused(message)| field_error("module-delivery-refused", &message_text(ctx, message)));
    let (tone, glyph) = stands.tone();
    let mut strip = Strip::new(ID, (Icon::GitBranch, glyph), "", detail(ctx, view, stands))
      .title_parts(title(ctx, view, stands))
      .tone(tone);
    if let Some(kind) = stands.offers() {
      strip = strip.action(self.button(ctx, view, (stands, kind), state.is_pending()));
    }
    let dialog = self
      .asking
      .get()
      .then(|| self.dialog(ctx, &props, view, state.is_pending()));
    column
      .describe("state", stands.name())
      .child(strip)
      .with_children(refused)
      .with_children(dialog)
  }
}

impl ModuleDelivery {
  /// **Deliver**: off, with why, while a run of the module works; turning while the merge is made.
  fn button(
    &self,
    ctx: &Ctx,
    view: &DeliveryView,
    (stands, kind): (Stands, ButtonKind),
    pending: bool,
  ) -> ActionButton {
    let button = ActionButton::new(DELIVER, kind, ctx.t("modules.delivery.deliver")).icon(Icon::GitMerge);
    if pending {
      return button.loading();
    }
    if stands == Stands::AtWork {
      return button.unavailable(ctx.t_args("modules.delivery.waitsForRuns", [("tasks", view.at_work.join(", "))]));
    }
    let asking = self.asking.clone();
    button.on_click(move || asking.set(true))
  }

  /// What the delivery does, asked before it is done.
  fn dialog(&self, ctx: &Ctx, props: &DeliveryProps, view: &DeliveryView, pending: bool) -> Element {
    let named = [("branch", view.own.branch.to_string()), ("into", into(ctx, view))];
    let confirm = ActionButton::new(
      "module-deliver-confirm",
      ButtonKind::Primary,
      ctx.t("modules.delivery.deliver"),
    )
    .icon(Icon::GitMerge);
    let confirm = if pending {
      confirm.loading()
    } else {
      let (deliver, args) = (
        self.deliver.clone(),
        (props.project.open.repo_root.clone(), props.page.module.id()),
      );
      confirm.on_click(move || deliver.run(args.clone()))
    };
    let what = if view.open > 0 {
      ctx.t_args(
        "modules.delivery.confirmOpen",
        [
          ("branch", view.own.branch.to_string()),
          ("into", into(ctx, view)),
          ("open", view.open.to_string()),
        ],
      )
    } else {
      ctx.t_args("modules.delivery.confirmWhat", named.clone())
    };
    Dialog::new(
      "module-deliver-dialog",
      Icon::GitMerge,
      ctx.t_args("modules.delivery.confirmTitle", named),
      ctx.t("modules.delivery.cancel"),
    )
    .tone(Tone::Neutral)
    .what(what)
    .action(confirm)
    .render(ctx, &self.asking)
  }
}

/// The base branch by its name, or *the checked-out branch* for a module that names none.
fn into(ctx: &Ctx, view: &DeliveryView) -> String {
  view
    .into
    .as_ref()
    .map_or_else(|| ctx.t("modules.delivery.checkedOut").to_string(), ToString::to_string)
}

/// `module/webhooks-v2 → main` in Geist Mono with an arrow between them, and how it stands in a badge after them.
fn title(ctx: &Ctx, view: &DeliveryView, stands: Stands) -> Vec<Element> {
  let mono = |id: &str, words: &str, color: PaletteColor| -> Element {
    Text::new(words)
      .id(id.to_owned())
      .variant(ExtraTypography::PropertyMono.role())
      .color(color)
      .nowrap()
      .into()
  };
  let state = match stands {
    Stands::Accepting => Some(("modules.delivery.state.accepting", BadgeTone::Neutral)),
    Stands::Accepted => Some(("modules.delivery.state.accepted", BadgeTone::Success)),
    Stands::NotPassed => Some(("modules.delivery.state.notPassed", BadgeTone::Warning)),
    Stands::Ceiling => Some(("modules.delivery.state.ceiling", BadgeTone::Warning)),
    Stands::Delivered => Some(("modules.delivery.state.delivered", BadgeTone::Success)),
    Stands::Nothing | Stands::AtWork | Stands::Open | Stands::Accept | Stands::NoAcceptance => None,
  };
  let mut parts = vec![
    mono(
      "module-delivery-branch",
      view.own.branch.as_str(),
      PaletteColor::TextPrimary,
    ),
    icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted).into(),
    mono("module-delivery-into", &into(ctx, view), PaletteColor::TextSecondary),
  ];
  if let Some((key, tone)) = state {
    parts.push(badge("module-delivery-state", tone, &ctx.t(key)).into());
  }
  parts
}

/// What the row's state rests on.
fn detail(ctx: &Ctx, view: &DeliveryView, stands: Stands) -> Arc<str> {
  let counts = [("merged", view.merged.to_string()), ("tasks", view.tasks.to_string())];
  match stands {
    Stands::Nothing => ctx.t("modules.delivery.detail.nothing"),
    Stands::AtWork => ctx.t_args(
      "modules.delivery.detail.atWork",
      [
        ("merged", view.merged.to_string()),
        ("tasks", view.tasks.to_string()),
        ("runs", view.at_work.join(", ")),
      ],
    ),
    Stands::Open => ctx.t_args("modules.delivery.detail.open", counts),
    Stands::Accept => ctx.t_args("modules.delivery.detail.accept", counts),
    Stands::Accepting => ctx.t_args("modules.delivery.detail.accepting", counts),
    Stands::NotPassed => ctx.t_args("modules.delivery.detail.notPassed", counts),
    Stands::Accepted => ctx.t_args("modules.delivery.detail.accepted", counts),
    Stands::NoAcceptance => ctx.t_args("modules.delivery.detail.noAcceptance", counts),
    Stands::Ceiling => ctx.t_args("modules.delivery.detail.ceiling", counts),
    Stands::Delivered => match &view.delivered {
      Some(delivered) => ctx.t_args(
        "modules.delivery.detail.delivered",
        [
          ("into", delivered.into.to_string()),
          ("commit", delivered.merge.to_string().chars().take(7).collect()),
          ("time", moment(ctx, delivered.at)),
        ],
      ),
      None => ctx.t("modules.delivery.detail.nothing"),
    },
  }
}
