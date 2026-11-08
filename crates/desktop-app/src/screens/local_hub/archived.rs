//! What archived looks like in the hub (`Agents · Archived` L13 `MXma9`, `Agents · Archived resource · Restore` L14
//! `KnPGN`): with nothing picked the panel says what archived means; an archived entry's panel says what archiving did
//! (the runs it stopped, which stay stopped) and what *Restore* does, and its one pinned action is *Restore*, a plain
//! action with no modal: restoring loses nothing and puts it back in the lists and pickers as it was.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{agents::local_hub::StoppedRun, local_archive, messages::Message};

use super::{
  model::Entry,
  panel::{chips_of, frame},
  words::{archived_at, kind_where_key},
};
use crate::{
  screens::team_hub::{
    bar::bar,
    panel::{body, head_named},
  },
  services::Services,
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The panel while nothing archived is picked (`Nothing picked` `gd374` in L13): a 20 glyph, the title and the line
/// under it, centered in the panel with no tile of their own.
pub(super) fn nothing_picked(ctx: &Ctx) -> Element {
  Column::new()
    .id("local-hub-archived-none")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(NOTHING_PICKED_SIDES, 0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::Archive, IconSize::Panel, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("local_hub.archived.nothingTitle"))
        .id("local-hub-archived-none-title")
        .variant(HubText::RowName.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(&ctx.t("local_hub.archived.nothingDetail"))
        .id("local-hub-archived-none-line")
        .variant(HubText::ListNote.role())
        .width(Dimension::Pct(100.0))
        .text_align(TextAlign::Center),
    )
    .into()
}

/// The panel's room at each side of the explanation (`gd374`).
const NOTHING_PICKED_SIDES: f32 = 60.0;

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub root: std::path::PathBuf,
  pub project: ProjectId,
  pub entry: Entry,
  pub stopped: Vec<StoppedRun>,
  pub now: Timestamp,
}
impl DevtoolsInspectable for Props {}

/// The panel of an archived entry, with *Restore*.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-restore-{:?}", props.entry.what);
  ctx.mount_keyed::<Restore>(&key, props)
}

struct Restore {
  restore: FutureAction<(), (), Message>,
}

impl Component for Restore {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    Self {
      restore: ctx.future_action(move |(): ()| {
        let (services, props) = (services.clone(), props.clone());
        async move {
          let what = props.entry.what;
          local_archive::restore(&props.root, what).await?;
          for change in what.changes(Some(props.entry.kind)) {
            services.changes.record(props.project, *change);
          }
          Ok(())
        }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let entry = &props.entry;
    let state = self.restore.state().get();
    let said = archived_at(ctx, props.now, entry.archived.unwrap_or(props.now));
    let body = body()
      .child(head_named(
        &entry.name,
        (ctx.t(kind_where_key(entry.kind)).to_string(), Some(said.to_string())),
        None,
      ))
      .child(chips_of(
        vec![(ctx.t("local_hub.chip.archived"), BadgeTone::Neutral)],
        None,
      ))
      .child(notice(ctx, entry, &props.stopped))
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !state.is_pending())
          .map(|error| Text::new(&message_text(ctx, error)).variant(HubText::PanelHint.role())),
      );
    let action = self.restore.clone();
    let restore = ActionButton::new(
      "archive-restore",
      ButtonKind::Primary,
      ctx.t_args("local_hub.archived.restore", [("name", entry.name.as_str())]),
    )
    .icon(Icon::ArchiveRestore)
    .pinned();
    let restore = if state.is_pending() {
      restore.loading()
    } else {
      restore.on_click(move || action.run(()))
    };
    frame(
      entry.what,
      body,
      Some(bar(
        vec![restore.into()],
        Some(ctx.t("local_hub.archived.restoreHint").to_string()),
      )),
    )
  }
}

/// What archiving did and what Restore does (`Notice` in L14).
fn notice(ctx: &Ctx, entry: &Entry, stopped: &[StoppedRun]) -> Column {
  let words = if stopped.is_empty() {
    ctx.t_args("local_hub.archived.stoppedNone", [("name", entry.name.as_str())])
  } else {
    let runs: Vec<String> = stopped
      .iter()
      .map(|run| {
        ctx
          .t_args(
            "local_hub.archived.run",
            [("key", run.key.as_str()), ("title", run.title.as_str())],
          )
          .to_string()
      })
      .collect();
    let named: Vec<&str> = runs.iter().map(String::as_str).collect();
    ctx.t_args(
      if stopped.len() == 1 {
        "local_hub.archived.stoppedOne"
      } else {
        "local_hub.archived.stopped"
      },
      [
        ("count", stopped.len().to_string()),
        (
          "runs",
          crate::screens::team_hub::archive::words::listed(ctx, &named).to_string(),
        ),
        ("name", entry.name.clone()),
      ],
    )
  };
  Column::new()
    .id("local-hub-archived-notice")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t("local_hub.archived.title"))
        .id("local-hub-archived-title")
        .variant(HubText::PanelLabel.role()),
    )
    .child(
      Text::new(&words)
        .id("local-hub-archived-body")
        .variant(HubText::Value.role())
        .width(Dimension::Pct(100.0)),
    )
}
