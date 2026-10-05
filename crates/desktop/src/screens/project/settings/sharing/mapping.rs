//! The keys tasks had before (10.22, 10.26 C): after a join, the notice that says what happened and
//! *Keys before joining*, which stays in Sharing until *Make local*; after *Make local*, the notice
//! with the *Not shared* tasks that returned to the list and their new keys.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_store::JoinedKey;

use super::{Actions, Facts, SharingTab, local::CONTROL_HEIGHT};
use crate::{
  navigation::route::Page,
  sharing::{
    feed::{JustJoined, ServerFeed},
    project::state::ProjectSharing,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    notice::Notice,
  },
  workspace::OpenProject,
};

/// The mapping's columns (`YEcbT`, `Gwv2T`, `r97Nc` in `m9omas`): the key before, the arrow, the
/// key now; the title and its note take the rest.
const BEFORE_WIDTH: f32 = 90.0;
const ARROW_WIDTH: f32 = 30.0;
const NOW_WIDTH: f32 = 112.0;
const TASK_WIDTH: f32 = 242.0;
/// A mapping row's height (`Y6ztLN` row).
const ROW_HEIGHT: f32 = 44.0;

impl SharingTab {
  /// *You joined ORC on Kontur team* (`CwBT7`), right after the join, until dismissed.
  pub(super) fn joined_notice(&self, ctx: &mut Ctx, sharing: &ProjectSharing, facts: &Facts) -> Option<Element> {
    let feed = ServerFeed::of(ctx);
    let root = ctx.props::<super::SharingProps>().open.repo_root.clone();
    let JustJoined(Some((joined_root, dropped))) = feed.joined.get() else {
      return None;
    };
    if joined_root != root {
      return None;
    }
    let key = sharing.link.key.as_str();
    let uploaded = facts.joined.iter().filter(|row| !row.not_shared).count();
    let claimed = facts.joined.iter().filter(|row| row.claimed_run.is_some()).count();
    let kept = facts.joined.len() - uploaded;
    let detail = ctx.t_args(
      "sharing.joined.detail",
      [
        ("uploaded", uploaded.to_string().as_str()),
        ("claimed", claimed.to_string().as_str()),
        ("kept", kept.to_string().as_str()),
        ("key", key),
      ],
    );
    let mut notice = Notice::new(
      Icon::CircleCheck,
      ctx.t_args(
        "sharing.joined.heading",
        [("key", key), ("server", sharing.server_name())],
      ),
      detail,
    )
    .emphasized()
    .id("sharing-joined");
    if !dropped.is_empty() {
      let links: Vec<String> = dropped
        .iter()
        .map(|(task, blocking)| {
          ctx
            .t_args(
              "sharing.joined.link",
              [("task", task.as_str()), ("blocking", blocking.as_str())],
            )
            .to_string()
        })
        .collect();
      notice = notice.below(
        Text::new(&ctx.t_args(
          "sharing.joined.dropped",
          [("links", links.join(&ctx.t("common.listSeparator")).as_str())],
        ))
        .variant(ExtraTypography::Detail.role()),
      );
    }
    let joined = feed.joined;
    Some(
      notice
        .below(self.notice_actions(ctx, move || joined.set(JustJoined(None))))
        .into(),
    )
  }

  /// *Keys before joining* (`O92PE4`): every local task's key before the join and now.
  pub(super) fn mapping(&self, ctx: &Ctx, facts: &Facts) -> Option<Element> {
    let example = facts.joined.iter().find(|row| !row.not_shared)?;
    let description = ctx.t_args(
      "sharing.mapping.detail",
      [
        ("key", example.key.project().as_str()),
        ("before", example.former.as_str()),
        ("now", example.key.to_string().as_str()),
      ],
    );
    let mut table = Column::new().width(Dimension::Pct(100.0)).child(headings(ctx));
    for row in &facts.joined {
      table = table.child(mapping_row(ctx, row));
    }
    Some(
      Column::new()
        .id("sharing-mapping")
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Lg)
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t("sharing.mapping.heading")).variant(ExtraTypography::SectionHeading.role()))
        .child(
          Text::new(&description)
            .variant(ExtraTypography::Detail.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(table)
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(icon(Icon::GitMerge, IconSize::Small, PaletteColor::TextSecondary))
            .child(
              Text::new(&ctx.t("sharing.mapping.branches"))
                .variant(ExtraTypography::CellLabel.role())
                .flex(1.0),
            ),
        )
        .into(),
    )
  }

  /// *Kontur is local again* (10.26 C) after Make local, with the *Not shared* tasks that returned.
  pub(super) fn made_local_notice(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    facts: &Facts,
    actions: &Actions,
  ) -> Option<Element> {
    let state = actions.make_local.state().get();
    let shown = state.is_fulfilled() && !self.notice_dismissed.get();
    let returned = state.data.filter(|_| shown)?.0;
    let project = open.project.name().as_str();
    let key = open.project.key().as_str();
    let all = facts.tasks.saturating_sub(returned.len()).to_string();
    let detail = match returned.len() {
      0 => ctx.t_args("sharing.madeLocal.detailNone", [("count", all.as_str()), ("key", key)]),
      1 => ctx.t_args("sharing.madeLocal.detail.one", [("count", all.as_str()), ("key", key)]),
      kept => ctx.t_args(
        "sharing.madeLocal.detail.other",
        [
          ("count", all.as_str()),
          ("key", key),
          ("kept", kept.to_string().as_str()),
        ],
      ),
    };
    let mut rows = Column::new().width(Dimension::Pct(100.0));
    for task in returned.iter() {
      rows = rows.child(key_row(
        ctx,
        (&task.before.to_string(), Some(&task.after.to_string())),
        task.title.as_str(),
        None,
      ));
    }
    let dismissed = self.notice_dismissed.clone();
    Some(
      Notice::new(
        Icon::HardDrive,
        ctx.t_args("sharing.madeLocal.heading", [("project", project)]),
        detail,
      )
      .emphasized()
      .id("sharing-made-local")
      .below(rows)
      .below(self.notice_actions(ctx, move || dismissed.set(true)))
      .into(),
    )
  }

  /// *Open Tasks* and *Dismiss* under a notice.
  fn notice_actions(&self, ctx: &Ctx, dismiss: impl Fn() + Send + Sync + 'static) -> Element {
    let nav = self.nav.clone();
    Row::new()
      .padding_top(ExtraSpacing::Tight.role())
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(ctx.t("sharing.joined.openTasks"))
          .icon(Icon::ArrowRight)
          .id("sharing-open-tasks")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(move || nav.show(Page::TASKS))
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("sharing.joined.dismiss"))
          .id("sharing-notice-dismiss")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(dismiss)
          .render(ctx),
      )
      .into()
  }
}

fn headings(ctx: &Ctx) -> Element {
  let heading = |key: &str, width: f32| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::ColumnHeading.role())
      .width(Dimension::Px(width))
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(heading("sharing.mapping.before", BEFORE_WIDTH + ARROW_WIDTH))
    .child(heading("sharing.mapping.now", NOW_WIDTH))
    .child(heading("sharing.mapping.task", TASK_WIDTH))
    .into()
}

/// A local task's key before the join and now, with what happened to it.
fn mapping_row(ctx: &Ctx, row: &JoinedKey) -> Element {
  let note: Option<Arc<str>> = if row.not_shared {
    Some(ctx.t("sharing.mapping.notSharedNote"))
  } else {
    row
      .claimed_run
      .map(|run| ctx.t_args("sharing.mapping.claimed", [("number", run.get().to_string().as_str())]))
  };
  let now = (!row.not_shared).then(|| row.key.to_string());
  key_row(ctx, (&row.former, now.as_ref()), row.title.as_str(), note.as_deref())
}

/// `before` → `now` (or *Not shared* without one), the task's title and a note.
fn key_row(ctx: &Ctx, (before, now): (&str, Option<&String>), title: &str, note: Option<&str>) -> Element {
  let now: Element = match now {
    Some(key) => Text::new(key)
      .variant(ExtraTypography::MonoValue.role())
      .color(PaletteColor::TextPrimary)
      .into(),
    None => Row::new()
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Archive, IconSize::Small, PaletteColor::TextSecondary))
      .child(Text::new(&ctx.t("sharing.notShared.label")).variant(TypographyStyle::Caption))
      .into(),
  };
  Row::new()
    .id(format!("sharing-mapping-{before}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(before)
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Px(BEFORE_WIDTH)),
    )
    .child(Row::new().width(Dimension::Px(ARROW_WIDTH)).child(icon(
      Icon::ArrowRight,
      IconSize::Small,
      PaletteColor::TextSecondary,
    )))
    .child(Row::new().width(Dimension::Px(NOW_WIDTH)).child(now))
    .child(
      Text::new(title)
        .variant(ExtraTypography::FileName.role())
        .color(PaletteColor::TextPrimary)
        .width(Dimension::Px(TASK_WIDTH)),
    )
    .child(
      Text::new(note.unwrap_or_default())
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    )
    .into()
}
