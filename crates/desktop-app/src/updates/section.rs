//! Settings › General's *Updates* (`Updates` `C0AmG` in `01 · Settings · General · update ready` `wSMNG`), before
//! *This computer*: the update row in its state (`row`, `c69Eti`) with the release notes under it while *What's new* is
//! open, *Channel* (Stable · Beta), *Update automatically*, and *Last checked* with *Check now*. A route that asks for
//! it (the status bar's item, the ready notification) scrolls the group into view, with the notes open after an
//! update. While the settings are read their controls are bones.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::app_update::{Channel, UpdateSettings};
use orchester_workspace::display;
use parking_lot::Mutex;

use super::{
  UpdateAction, UpdateDispatch, UpdatesFacts,
  parts::{Parts, leading_link},
  row::{channel_name, update_row},
};
use crate::{
  routes::UpdatesShow,
  ui::{
    field::field_error,
    icon::Icon,
    markdown::LinkHandler,
    pressable::Focus,
    setting_rows::{SettingGroup, SettingRow},
    skeleton::{self, Skeleton},
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
  },
};

/// The group's element id.
pub const GROUP: &str = "settings-updates";
/// The last checked hint's bone while it is read (`ndTIT`).
const CHECKED_BONE: f32 = 132.0;

/// What [`UpdatesSection`] shows and where its presses go.
#[derive(Clone)]
pub struct SectionProps {
  pub facts: UpdatesFacts,
  pub dispatch: UpdateDispatch,
}

/// Props are compared by what the section shows; the dispatch goes to the same window.
impl PartialEq for SectionProps {
  fn eq(&self, other: &Self) -> bool {
    self.facts == other.facts
  }
}

impl DevtoolsInspectable for SectionProps {}

pub struct UpdatesSection {
  group: ElementRef,
  /// Whether the release notes show.
  notes_open: Signal<bool>,
  /// What the route last asked to show, which this section followed already.
  followed: Mutex<Option<UpdatesShow>>,
  /// The dispatch a link in the notes goes to, kept with one handler so the notes are laid out only when they change.
  links: Arc<Mutex<Option<UpdateDispatch>>>,
  on_link: LinkHandler,
}

impl Component for UpdatesSection {
  type Props = SectionProps;

  fn create(ctx: &mut Ctx) -> Self {
    let links: Arc<Mutex<Option<UpdateDispatch>>> = Arc::default();
    let to = links.clone();
    let on_link: LinkHandler = Arc::new(move |destination: &str, _event: &MouseEvent| {
      if let Some(dispatch) = to.lock().as_ref() {
        dispatch(UpdateAction::Open(destination.to_owned()));
      }
    });
    Self {
      group: ctx.element_ref(),
      notes_open: ctx.signal(false),
      followed: Mutex::new(None),
      links,
      on_link,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let SectionProps { facts, dispatch } = ctx.props::<SectionProps>().clone();
    *self.links.lock() = Some(dispatch.clone());
    self.follow(ctx, facts.show);
    let open = self.notes_open.clone();
    let parts = Parts {
      dispatch: dispatch.clone(),
      toggle_notes: Arc::new(move || open.update(|open| *open = !*open)),
      on_link: self.on_link.clone(),
    };
    let rows = [
      update_row(ctx, &facts, self.notes_open.get(), &parts),
      channel(ctx, &facts, &dispatch),
      automatic(ctx, facts.settings, &dispatch),
      last_checked(ctx, facts.last_checked, &parts),
    ];
    let group = SettingGroup::new(GROUP, ctx.t("settings.general.updates.title")).rows(rows);
    Column::new()
      .ref_element(self.group.clone())
      .width(Dimension::Pct(100.0))
      .child(group)
  }
}

impl UpdatesSection {
  /// Brings the group into view when a route newly asks for it, opening the notes when it asks for them.
  fn follow(&self, ctx: &mut Ctx, show: Option<UpdatesShow>) {
    let mut followed = self.followed.lock();
    if *followed == show {
      return;
    }
    *followed = show;
    let Some(show) = show else {
      return;
    };
    if show == UpdatesShow::WhatsNew && !self.notes_open.get_untracked() {
      self.notes_open.set(true);
    }
    ctx.scroll_into_view(&self.group);
  }
}

/// *Channel*: Stable or Beta (`Channel` `Z66oq`); what is wrong with the settings under it.
fn channel(ctx: &mut Ctx, facts: &UpdatesFacts, dispatch: &UpdateDispatch) -> Element {
  const ID: &str = "settings-update-channel";
  let control = match facts.settings {
    Some(settings) => {
      let segments = [Channel::Stable, Channel::Beta].map(|channel| {
        let dispatch = dispatch.clone();
        Segment::new(
          format!("{ID}-{}", channel.as_str()),
          SegmentFace::Text(channel_name(ctx, channel).into()),
          settings.channel == channel,
          move || dispatch(UpdateAction::Channel(channel)),
        )
      });
      segmented(&format!("{ID}-choice"), segments, Focus::Keyboard).into()
    }
    None => reading(ctx, ID),
  };
  let problem = facts
    .problem
    .as_ref()
    .map(|problem| field_error(&format!("{GROUP}-problem"), problem).into());
  SettingRow::new(ID, ctx.t("settings.general.updates.channel.title"))
    .hint(ctx.t("settings.general.updates.channel.hint"))
    .control(control)
    .below_all(problem)
    .render(ctx)
}

/// *Update automatically* (`Update automatically` `J94hd`).
fn automatic(ctx: &mut Ctx, settings: Option<UpdateSettings>, dispatch: &UpdateDispatch) -> Element {
  const ID: &str = "settings-update-automatic";
  let label = ctx.t("settings.general.updates.automatic.title");
  let control = match settings {
    Some(settings) => {
      let dispatch = dispatch.clone();
      Toggle::new(ID, ToggleKind::Switch, settings.automatic, label.clone())
        .hide_label()
        .on_change(move |on| dispatch(UpdateAction::Automatic(on)))
        .into()
    }
    None => reading(ctx, ID),
  };
  SettingRow::new(ID, label)
    .hint(ctx.t("settings.general.updates.automatic.hint"))
    .control(control)
    .render(ctx)
}

/// *Last checked*: when Orchester Cloud last answered, and *Check now* (`Last checked` `qSk2x`).
fn last_checked(ctx: &mut Ctx, at: Option<Option<orchester_core::time::Timestamp>>, parts: &Parts) -> Element {
  const ID: &str = "settings-update-checked";
  let row = SettingRow::new(ID, ctx.t("settings.general.updates.lastChecked.title"));
  let row = match at {
    Some(Some(at)) => row.hint(ctx.t_args(
      "settings.general.updates.lastChecked.at",
      [("time", display::clock(at))],
    )),
    Some(None) => row.hint(ctx.t("settings.general.updates.lastChecked.never")),
    None => row.hint_reading(CHECKED_BONE),
  };
  let words = ctx.t("settings.general.updates.checkNow");
  row
    .control(leading_link(
      &format!("{ID}-check-now"),
      (Icon::RefreshCw, &words),
      parts.on(UpdateAction::CheckNow),
    ))
    .render(ctx)
}

/// A control's bone while the settings are read.
fn reading(ctx: &mut Ctx, id: &str) -> Element {
  Skeleton::new(format!("{id}-reading"), ctx.t("settings.reading"))
    .inline()
    .show(ctx, skeleton::switch())
}
