//! Clean up's one modal (X9BwMn): the exact snapshot to discard, its estimated space, and what stays.
use lurq::{
  app::{ctx::Ctx, events::is_synthetic_input, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};

use super::{StoragePage, bytes, work};
use crate::{
  services::storage::Disk,
  theme::{DialogText, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, line},
    icon::Icon,
  },
};
/// The modal's list scrolls when a project has many ended runs.
const LIST_HEIGHT: f32 = 240.0;

pub fn dialog(ctx: &mut Ctx, page: &StoragePage, disk: Option<&Disk>) -> Option<Element> {
  if !page.modal.get() {
    return None;
  }
  let plan = page.prepare.state().get().data?;
  let size = work::total(&plan);
  let list = candidates(ctx, &plan);
  let open = page.modal.clone();
  let remove = page.remove.clone();
  let person_only = plan.0.iter().any(|candidate| candidate.person_only);
  let run = move || {
    if person_only && is_synthetic_input() {
      return;
    }
    if open.get_untracked() && !remove.state().get_untracked().is_pending() {
      open.set(false);
      remove.run(plan.clone());
    }
  };
  let submit = std::sync::Arc::new(run);
  let click = submit.clone();
  let action = ActionButton::new(
    "storage-confirm-remove",
    ButtonKind::Danger,
    ctx.t_args(
      "settings.project.runs.storage.clean_up.confirm.action",
      [("size", bytes(ctx, size))],
    ),
  )
  .on_click(move || click());
  let after = disk
    .and_then(|disk| disk.guard.disk)
    .and_then(|d| size.map(|size| d.free.saturating_add(size)));
  let count = page.prepare.state().get().data.as_ref().map_or(0, |p| p.0.len());
  let mut modal = Dialog::new(
    "storage-confirm",
    Icon::Trash2,
    ctx.t_args(
      if count == 1 {
        "settings.project.runs.storage.clean_up.confirm.titleOne"
      } else {
        "settings.project.runs.storage.clean_up.confirm.title"
      },
      [("count", count.to_string())],
    ),
    ctx.t("settings.project.runs.storage.cancel"),
  )
  .part(Text::new(&ctx.t("settings.project.runs.storage.clean_up.confirm.lead")).variant(DialogText::Line.role()))
  .part(ScrollVertical::new(list).max_height(Dimension::Px(LIST_HEIGHT)))
  .part(freed(ctx, disk, size, after))
  .part(line(
    "storage-confirm-stays",
    Icon::GitBranch,
    &ctx.t("settings.project.runs.storage.clean_up.confirm.stays"),
  ))
  .part(line(
    "storage-confirm-untouched",
    Icon::ShieldCheck,
    &ctx.t("settings.project.runs.storage.clean_up.confirm.untouched"),
  ))
  .action(action);
  if person_only {
    modal = modal.person_only(crate::ui::message::message_text(
      ctx,
      &orchester_workspace::messages::Message::new("appMcp.errors.uncommittedChanges"),
    ));
  }
  let modal = modal.render(ctx, &page.modal);
  let open = page.modal.clone();
  Some(
    Column::new()
      .on_key_down(move |event: lurq::app::events::KeyboardEvent| {
        if open.get_untracked() && event.key == "Enter" && !event.text_input_focused {
          submit();
          event.prevent_default();
        }
      })
      .child(modal)
      .into(),
  )
}

fn candidates(ctx: &Ctx, plan: &work::Plan) -> Column {
  let mut list = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
  for candidate in &plan.0 {
    list = list.child(
      Text::new(&ctx.t_args(
        "settings.project.runs.storage.clean_up.confirm.run",
        [
          ("title", candidate.title.clone()),
          ("number", candidate.run.number().get().to_string()),
        ],
      ))
      .variant(ExtraTypography::NoteStrong.role()),
    );
    for (tree, size) in candidate.left.worktrees.iter().zip(&candidate.sizes) {
      list = list.child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&tree.path)
              .variant(ExtraTypography::MonoSmall.role())
              .flex(1.0),
          )
          .child(Text::new(&bytes(ctx, *size)).variant(ExtraTypography::MonoSmall.role())),
      );
    }
  }
  list
}

fn freed(ctx: &Ctx, disk: Option<&Disk>, size: Option<u64>, after: Option<u64>) -> Row {
  line(
    "storage-confirm-frees",
    Icon::HardDrive,
    &ctx.t_args(
      "settings.project.runs.storage.clean_up.confirm.frees",
      [
        ("size", bytes(ctx, size).to_string()),
        ("after", bytes(ctx, after).to_string()),
        (
          "keep",
          disk.map_or_else(
            || ctx.t("settings.project.runs.storage.unknown").to_string(),
            |d| d.guard.keep_free.gigabytes().to_string(),
          ),
        ),
      ],
    ),
  )
}
