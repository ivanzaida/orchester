//! The header's *Leads* (`Leads` `fcVPL`, `TfIKc`, `j2rhWs` in `Parts · Header` `X93D8T` b–d; `p8qWl` in the header
//! `Zbwg0` of 04 `OKtX2`, as in 05 and 06): what the conversation leads, read from its subscriptions
//! (`orchester_workspace::orchestrator::watching::ChatLeads`), after a 13 `eye` and *Leads* in 12 `text_muted`, all 6
//! apart and 8 before the header's *New*. A chip per target in the order it was kept ([`chip`]); those the header has
//! no room for fold into *+N* at the end (`R8tXP`): a 22 high round chip in a `border` stroke, its words 11.5
//! `text_secondary`, whose tooltip names them. *+N* opens nothing yet: the `/watching` answer it is to open comes with
//! ORC-775, so it has no pointer or hover of a control. Nothing shows while the conversation leads nothing (a), nor
//! while its subscriptions cannot be read, which the log says.
//!
//! What fits is learned from the layout: the header's free room is a flexible spacer before the group, and each chip
//! reports its width. While the spacer has no room left a chip folds; a folded chip comes back once the spacer has room
//! for it and the gap before it. At least one chip shows. The fold holds while the pointer rests on a chip, whose ×
//! widens it.

mod chip;

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::{Chat, WatchTarget, watch::NamedTarget};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::{
    watch::{StopBy, StopWhat, stop_in},
    watching::{ChatLeads, WatchReads},
  },
};

use self::chip::{ChipProps, Hold, LeadChipView};
use crate::{
  project::settled_read::settled_read,
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    tooltip::Tooltip,
  },
};

const ID: &str = "orchestrator-leads";
/// A Leads chip's height, *+N*'s too (`VFXga`, `R8tXP` in `X93D8T`).
const CHIP_HEIGHT: f32 = 22.0;
/// Less room than this is none: the layout rounds.
const NO_ROOM: f32 = 0.5;

/// The header, which a chip's tooltip hangs under, its end at the header's end (`nzz9r` in `X93D8T` d).
#[derive(Clone)]
struct Head(ElementRef);

/// The header is the screen's for as long as it shows: two compare equal.
impl PartialEq for Head {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

#[derive(Clone, PartialEq)]
struct LeadsProps {
  root: Arc<Path>,
  chat: Chat,
  /// How often the project's tasks were saved: a module renamed or a task moved reads again.
  saves: u64,
  head: Head,
}

impl DevtoolsInspectable for LeadsProps {}

/// The Leads of conversation `chat`, kept in the project at `root` whose tasks were saved `saves` times; their
/// tooltips hang under `head`, the header.
pub(super) fn leads(ctx: &mut Ctx, (root, saves): (&Path, u64), chat: &Chat, head: &ElementRef) -> Element {
  ctx.mount_keyed::<Leads>(
    &format!("{ID}-{}", chat.id()),
    LeadsProps {
      root: Arc::from(root),
      chat: chat.clone(),
      saves,
      head: Head(head.clone()),
    },
  )
}

/// What a conversation leads, as read.
#[derive(Clone, PartialEq)]
struct Led(Arc<ChatLeads>);

impl DevtoolsInspectable for Led {}

/// A stop the person asked for with ×.
struct Stopping {
  root: Arc<Path>,
  chat: Chat,
  target: NamedTarget,
}

/// A target whose subscription ended.
#[derive(Clone, PartialEq)]
struct Stopped(WatchTarget);

impl DevtoolsInspectable for Stopped {}

/// A stop that was not applied, and why: its chip's tooltip says so.
#[derive(Clone, PartialEq)]
struct Refused {
  target: WatchTarget,
  reason: Message,
}

impl DevtoolsInspectable for Refused {}

type StopAction = FutureAction<Stopping, Stopped, Refused>;

struct Leads {
  fit: Fit,
  /// The group of the label and the chips, which changes width as chips fold.
  group: ElementRef,
  stop: StopAction,
}

impl Component for Leads {
  type Props = LeadsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let runners = Services::of(ctx).runners;
    let stop: StopAction = ctx.future_action(move |Stopping { root, chat, target }| {
      let runners = runners.clone();
      async move {
        let (watched, kept) = (target.target, root.clone());
        let stopped = background::run(move || {
          stop_in(&kept, &chat, &StopWhat::Targets(vec![target]), StopBy::Chip).map_err(|error| error.message())
        })
        .await
        .and_then(|stopped| stopped);
        // Refused or not, what the conversation leads is read again: a refusal means it changed meanwhile.
        runners.chat_changed(&root);
        match stopped {
          Ok(_) => Ok(Stopped(watched)),
          Err(reason) => {
            tracing::warn!(?reason, "a Leads chip's subscription was not stopped");
            Err(Refused {
              target: watched,
              reason,
            })
          }
        }
      }
    });
    Self {
      fit: Fit::new(ctx),
      group: ctx.element_ref(),
      stop,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LeadsProps>().clone();
    let led = read(ctx, &props);
    let spacer = Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .ref_element(self.fit.spacer.clone());
    let root = Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .align_items(Alignment::Center)
      .child(spacer);
    let Some(led) = led.filter(|led| !led.0.targets.is_empty()) else {
      return root;
    };
    let shown = self.fit.hold(&led.0);
    let (on_spacer, on_group) = (self.fit.clone(), self.fit.clone());
    self.fit.spacer.observe_rect(move |_| on_spacer.adjust());
    self.group.observe_rect(move |_| on_group.adjust());
    let hold = Hold {
      head: props.head.0.clone(),
      fit: self.fit.clone(),
      stop: self.stopper(&props),
    };
    let failed = self.stop.state().get().error;
    let chips: Vec<Element> = led.0.targets[..shown]
      .iter()
      .map(|lead| {
        let id = format!("{ID}-{}", slug(lead.target.target));
        let failed = failed
          .as_ref()
          .filter(|refused| refused.target == lead.target.target)
          .map(|refused| refused.reason.clone());
        ctx.mount_keyed::<LeadChipView>(
          &id.clone(),
          ChipProps {
            id,
            lead: lead.clone(),
            failed,
            hold: hold.clone(),
          },
        )
      })
      .collect();
    let more = more(ctx, &led.0, shown);
    root.child(
      Row::new()
        .id(ID)
        .ref_element(self.group.clone())
        .flex_shrink(0.0)
        .padding(Padding::new().right(SpacingSize::Sm))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Eye, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("orchestrator.leads.label"))
            .id(format!("{ID}-label"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .nowrap(),
        )
        .with_children(chips)
        .with_children(more),
    )
  }
}

impl Leads {
  /// Stops the subscription of a target `props.chat` leads, once the stop before has ended.
  fn stopper(&self, props: &LeadsProps) -> Arc<dyn Fn(NamedTarget) + Send + Sync> {
    let (stop, root, chat) = (self.stop.clone(), props.root.clone(), props.chat.clone());
    Arc::new(move |target| {
      stop.run_if_idle(Stopping {
        root: root.clone(),
        chat: chat.clone(),
        target,
      });
    })
  }
}

/// What `props.chat` leads, read off the UI thread again whenever the conversation changed or the project's tasks
/// change, never beside a reading that has not ended (`settled_read`); the last reading stays while it is read again.
/// `None` until it is read, and while it cannot be.
fn read(ctx: &mut Ctx, props: &LeadsProps) -> Option<Led> {
  let services = Services::of(ctx);
  let own = super::data::own_changes(ctx, (&services.runners, &props.root), Some(props.chat.id()));
  let hold = services.hold;
  let deps = (props.root.clone(), props.chat.id(), own, props.saves);
  let state = settled_read(ctx, "orchestrator leads", deps, move |(root, chat, ..)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || {
        let read = ProjectDb::open_read_only(&root).and_then(|db| WatchReads::read(&db, &root));
        read
          .map(|reads| Led(Arc::new(reads.chat_leads(chat))))
          .map_err(|error| {
            tracing::error!(%error, "cannot read what the conversation leads");
            messages::store_error(&error)
          })
      })
      .await
      .and_then(|read| read)
    }
  });
  state.data.filter(|_| state.error.is_none())
}

/// *+N* after the chips shown, while `shown` of `leads` leave some folded; its tooltip names them.
fn more(ctx: &mut Ctx, leads: &ChatLeads, shown: usize) -> Option<Element> {
  let folded: Vec<String> = leads.targets[shown..]
    .iter()
    .map(|lead| message_text(ctx, &lead.target.name).to_string())
    .collect();
  if folded.is_empty() {
    return None;
  }
  let id = format!("{ID}-more");
  let pill = Row::new()
    .id(id.clone())
    .height(Dimension::Px(CHIP_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .rounded(ExtraRadius::Full.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t_args("orchestrator.leads.more", [("count", folded.len().to_string())]))
        .variant(OrchestratorText::LeadsChip.role())
        .nowrap(),
    );
  let tip = ctx.t_args("orchestrator.leads.moreTip", [("targets", folded.join(", "))]);
  Some(Tooltip::new(id, tip).end().wrap(ctx, pill))
}

/// A target as an element id says it.
fn slug(target: WatchTarget) -> String {
  match target {
    WatchTarget::Task(id) => format!("task-{id}"),
    WatchTarget::Module(id) => format!("module-{id}"),
    WatchTarget::WithoutModule => "without-module".to_owned(),
    WatchTarget::Project => "project".to_owned(),
  }
}

/// The targets the chips were laid out for.
#[derive(Clone, Default, PartialEq)]
struct Targets(Vec<WatchTarget>);

impl DevtoolsInspectable for Targets {}

/// Each target's chip as wide as it was laid out without its ×.
#[derive(Clone, Default, PartialEq)]
struct Widths(Vec<(WatchTarget, f32)>);

impl DevtoolsInspectable for Widths {}

/// The chip the pointer rests on.
#[derive(Clone, Default, PartialEq)]
struct Resting(Option<WatchTarget>);

impl DevtoolsInspectable for Resting {}

/// How many chips fit the header, learned from its layout.
#[derive(Clone)]
struct Fit {
  targets: Signal<Targets>,
  /// How many of them show; the rest fold into *+N*.
  shown: Signal<usize>,
  widths: Signal<Widths>,
  resting: Signal<Resting>,
  /// The header's free room before the group.
  spacer: ElementRef,
}

impl Fit {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      targets: ctx.signal(Targets::default()),
      shown: ctx.signal(0),
      widths: ctx.signal(Widths::default()),
      resting: ctx.signal(Resting::default()),
      spacer: ctx.element_ref(),
    }
  }

  /// How many of `leads`' chips show; all of them again once its targets changed. A chip that went while the pointer
  /// rested on it (its × stopped it) no longer holds the fold: it never saw the pointer leave.
  fn hold(&self, leads: &ChatLeads) -> usize {
    let targets: Vec<WatchTarget> = leads.targets.iter().map(|lead| lead.target.target).collect();
    let total = targets.len();
    if self.targets.get_untracked().0 != targets {
      if let Some(gone) = self
        .resting
        .get_untracked()
        .0
        .filter(|resting| !targets.contains(resting))
      {
        self.rest(gone, false);
      }
      self.targets.set(Targets(targets));
      self.shown.set(total);
    }
    self.shown.get().clamp(1, total)
  }

  /// Folds a chip while the header has no room left, or brings the next one back once it has room for it.
  fn adjust(&self) {
    if self.resting.get_untracked().0.is_some() || !self.spacer.is_attached() {
      return;
    }
    let (room, shown) = (self.spacer.width(), self.shown.get_untracked());
    if room < NO_ROOM {
      if shown > 1 {
        self.shown.set(shown - 1);
      }
      return;
    }
    let Some(next) = self.targets.get_untracked().0.get(shown).copied() else {
      return;
    };
    let width = self
      .widths
      .get_untracked()
      .0
      .iter()
      .find(|(target, _)| *target == next)
      .map_or(0.0, |(_, width)| *width);
    if room > width + ExtraSpacing::Snug.value() + NO_ROOM {
      self.shown.set(shown + 1);
    }
  }

  /// `target`'s chip was laid out `width` wide without its ×.
  fn record(&self, target: WatchTarget, width: f32) {
    let mut widths = self.widths.get_untracked();
    match widths.0.iter_mut().find(|(known, _)| *known == target) {
      Some((_, known)) if (*known - width).abs() < NO_ROOM => return,
      Some((_, known)) => *known = width,
      None => widths.0.push((target, width)),
    }
    self.widths.set(widths);
  }

  /// The pointer rests on `target`'s chip, or left it.
  fn rest(&self, target: WatchTarget, resting: bool) {
    let now = self.resting.get_untracked().0;
    if resting && now != Some(target) {
      self.resting.set(Resting(Some(target)));
    } else if !resting && now == Some(target) {
      self.resting.set(Resting(None));
    }
  }
}
