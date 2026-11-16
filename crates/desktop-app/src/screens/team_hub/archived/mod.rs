//! What the team archived, in the hub's *Archived* filter (`Agents · Archived · admin · Restore` A36 `qvMZD`, `·
//! member` M7 `Ml7HN`; the local hub's L13, L14 in [`crate::screens::local_hub`]): with nothing picked the panel says
//! what archived means; an archived resource's panel is read-only: what archiving did (the runs it stopped, which stay
//! stopped) and what *Restore* does, and what the team holds of it as values. An admin's one pinned action is
//! *Restore*, a plain action with no modal: restoring loses nothing, and the team server clears the archived mark for
//! the whole team (`PATCH` with `archived: false`). While it is sent, and once the server took it until the hub reads
//! it again, *Restore* stays pending and the resource stays here; read again, it leaves *Archived*. A refusal says why
//! under the read-only part. A member reads the same panel with no action bar: the line under the chips says only an
//! admin restores it, and who the admins are.

mod read_only;
mod words;

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  agents::team_archived::TeamArchived,
  sharing::{project::NotRestored, team::TeamConfiguration},
};

use super::{
  bar::bar,
  model::Entry,
  panel::{body, frame, head_named},
  words::archived_by,
};
use crate::{
  screens::{hub::team::admins, local_hub::archived::nothing_picked as nothing_picked_in},
  services::Services,
  team::lock::flat,
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::Icon,
  },
};

/// The panel while nothing archived is picked (`Nothing picked` `gd374` in L13).
pub(super) fn nothing_picked(ctx: &Ctx) -> Element {
  nothing_picked_in(
    ctx,
    "team-hub-archived-none",
    ("team_hub.archived.nothingTitle", "team_hub.archived.nothingDetail"),
  )
}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub root: PathBuf,
  pub entry: Entry,
  pub archived: TeamArchived,
  /// Whether the person may restore it (an admin, while the team server can be reached), and who the admins are.
  pub team: TeamConfiguration,
}
impl DevtoolsInspectable for Props {}

/// The panel of an archived resource of the team.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("team-hub-archived-{}", props.entry.id);
  ctx.mount_keyed::<Archived>(&key, props)
}

struct Archived {
  restore: FutureAction<(), (), NotRestored>,
}

impl Component for Archived {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    Self {
      restore: ctx.future_action(move |(): ()| {
        let (services, root, resource) = (services.clone(), props.root.clone(), props.entry.id);
        async move { services.servers.restore_resource(&root, resource).await }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      entry, archived, team, ..
    } = &props;
    let state = self.restore.state().get();
    let refused = state.error.as_ref().filter(|_| !state.is_pending());
    let body = body()
      .child(head_named(
        &entry.name,
        (
          ctx.t(entry.kind.where_key()).to_string(),
          Some(archived_by(ctx, &archived.archived).to_string()),
        ),
        None,
      ))
      .child(chips(ctx, archived))
      .with_children((!team.may_change).then(|| lock_line(ctx, team)))
      .child(words::notice(ctx, archived))
      .with_children(read_only::sections(ctx, archived))
      .with_children(refused.map(|refused| {
        Text::new(&words::refusal(ctx, &entry.name, refused))
          .id("team-hub-restore-error")
          .variant(HubText::PanelHint.role())
          .color(PaletteColor::Danger)
      }));
    let bar = team.may_change.then(|| self.bar(ctx, entry));
    frame(entry, body, bar)
  }
}

impl Archived {
  /// *Restore <name>* pinned at the bottom: pending while it is sent, and once the server took it until the hub reads
  /// it again and it leaves *Archived*.
  fn bar(&self, ctx: &Ctx, entry: &Entry) -> Element {
    let state = self.restore.state().get();
    let restore = ActionButton::new(
      "team-hub-restore",
      ButtonKind::Primary,
      ctx.t_args("team_hub.archived.restore", [("name", entry.name.as_str())]),
    )
    .icon(Icon::ArchiveRestore)
    .pinned();
    let restore = if state.is_pending() || state.is_fulfilled() {
      restore.loading()
    } else {
      let action = self.restore.clone();
      // A second press while the first restore is in flight leaves it to finish.
      restore.on_click(move || {
        action.run_if_idle(());
      })
    };
    bar(
      vec![restore.into()],
      Some(ctx.t("team_hub.archived.restoreHint").to_string()),
    )
  }
}

/// *Archived*, and at the chips' end the profiles that still name it (`Chips` in A36).
fn chips(ctx: &Ctx, archived: &TeamArchived) -> Row {
  let used = archived
    .sheet
    .as_ref()
    .map(|sheet| sheet.used_by())
    .filter(|names| !names.is_empty())
    .map(|names| ctx.t_args("team_hub.archived.wasUsedBy", [("names", names.join(", "))]));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(badge(
      "team-hub-chip-archived",
      BadgeTone::Danger,
      &ctx.t("team_hub.archived.chip"),
    ))
    .child(Row::new().flex(1.0))
    .with_children(used.map(|used| {
      Text::new(&used)
        .id("team-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

/// *Only team admins restore it: dana-k (Admin) and olha (Server admin).* under a member's chips (M7 `xXJXm`).
fn lock_line(ctx: &Ctx, team: &TeamConfiguration) -> Row {
  let words = ctx.t_args("team_hub.archived.lock", [("admins", admins(ctx, team))]);
  flat("team-hub-member-lock", &words)
}
