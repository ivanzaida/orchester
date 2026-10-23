//! *Resolve myself* (`Task · Resolve myself · in page` `fAcWt`): the base merged into the run's worktree waits for the
//! person. The card (`L3U2Al`) says where, with the worktree's path to copy (`h6jKOa`), *Open folder*, *Open in
//! editor*, *Back to the decision* and *I've resolved it*; under it the stages, the files that conflicted as they stand
//! now, read again every few seconds, and what *I've resolved it* checks (`D3QztR`).

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{integration::resolve::Resolution, merge_queue::QueueEvent};
use orchester_workspace::{
  display::{plain_path, short_revision},
  tasks::changes::run_worktree,
};

use super::{
  super::ready::changed::section,
  Act, View,
  card::base,
  integrate::{checks_card, conflicting},
  rows::{Look, Stage, check_row},
};
use crate::{
  services::Services,
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    clipboard::Clipboard,
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The worktree's box (`h6jKOa`).
const PATH_HEIGHT: f32 = 36.0;

/// The view while the person resolves `files` in the run's worktree, as `resolution` has them now.
pub(super) fn resolving(
  ctx: &mut Ctx,
  view: &View<'_>,
  (files, resolution): (&[String], &Resolution),
  back: &Signal<bool>,
) -> Vec<Element> {
  let worktree = run_worktree(&view.task.project.open.repo_root, view.run.id());
  let base = base(view.run);
  let pending = |id: &str, title: String, detail: String| check_row(id, Look::PENDING, (&title, &detail), None);
  let listed = files.join(", ");
  let checks = vec![
    pending(
      "task-delivery-resolve-markers",
      ctx.t("delivery.task.resolve.markers").to_string(),
      ctx.t_args("delivery.task.resolve.markersDetail", [("files", listed.as_str())]).to_string(),
    ),
    pending(
      "task-delivery-resolve-merges",
      ctx.t_args("delivery.task.resolve.merges", [("base", base)]).to_string(),
      ctx.t_args("delivery.task.resolve.mergesDetail", [("base", base)]).to_string(),
    ),
    pending(
      "task-delivery-resolve-check",
      ctx.t("delivery.task.resolve.check").to_string(),
      ctx.t("delivery.task.resolve.checkDetail").to_string(),
    ),
    pending(
      "task-delivery-resolve-back",
      ctx.t("delivery.task.resolve.back").to_string(),
      ctx.t("delivery.task.resolve.backDetail").to_string(),
    ),
  ];
  let when = section(
    ctx,
    ("task-delivery-resolve-when", "delivery.task.resolve.when"),
    None,
    checks_card(ctx, "task-delivery-resolve-checks", checks),
  );
  vec![
    card(ctx, view, &worktree, back),
    view.stages(ctx, Stage::Waiting),
    conflicting(ctx, files, Some(resolution)),
    when,
  ]
}

fn card(ctx: &mut Ctx, view: &View<'_>, worktree: &Path, back: &Signal<bool>) -> Element {
  let base = base(view.run);
  let synced = view.queue.delivery.as_ref().and_then(|delivery| {
    delivery.events.iter().rev().find_map(|recorded| match &recorded.event {
      QueueEvent::Synced { base, .. } => Some(short_revision(base.as_str()).to_owned()),
      _ => None,
    })
  });
  let body = ctx.t_args(
    "delivery.task.resolve.body",
    [
      ("base", base.to_owned()),
      ("revision", synced.unwrap_or_default()),
      ("branch", view.run.run.branch().as_str().to_owned()),
    ],
  );
  Column::new()
    .id("task-delivery-resolve")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(Icon::FilePenLine, IconSize::Lg, PaletteColor::Warning))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Hair.role())
            .child(
              Text::new(&ctx.t_args("delivery.task.resolve.title", [("base", base)]))
                .id("task-delivery-resolve-title")
                .variant(DeliveryText::CardTitle.role()),
            )
            .child(
              Text::new(&body)
                .variant(ExtraTypography::Prose.role())
                .color(PaletteColor::TextSecondary)
                .width(Dimension::Pct(100.0)),
            ),
        ),
    )
    .child(path_box(ctx, worktree))
    .child(actions(ctx, view, worktree, back))
    .into()
}

/// The worktree's path with a button that copies it.
fn path_box(ctx: &mut Ctx, worktree: &Path) -> Element {
  let path = plain_path(&worktree.to_string_lossy());
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  let copied = path.clone();
  let copy = IconButton::new("task-delivery-copy-path", Icon::Copy, ctx.t("delivery.task.resolve.copyPath"))
    .small()
    .on_click(move || {
      if !clipboard.copy(&copied) {
        tracing::warn!("the worktree's path could not be copied");
      }
    })
    .render(ctx);
  Row::new()
    .id("task-delivery-worktree")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PATH_HEIGHT))
    .padding(Padding::new().left(ExtraSpacing::Loose.role()).right(ExtraSpacing::Pair.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(Icon::FolderGit2, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&path)
        .id("task-delivery-worktree-path")
        .variant(DeliveryText::Branch.role())
        .flex(1.0)
        .nowrap(),
    )
    .child(copy)
    .into()
}

/// *Open folder*, *Open in editor*, *Back to the decision* and *I've resolved it*.
fn actions(ctx: &mut Ctx, view: &View<'_>, worktree: &Path, back: &Signal<bool>) -> Element {
  let folder = worktree.to_path_buf();
  let open_folder = ActionButton::new(
    "task-delivery-open-folder",
    ButtonKind::Secondary,
    ctx.t("delivery.queue.blocked.openFolder"),
  )
  .icon(Icon::FolderOpen)
  .on_click(move || opened("open-worktree", folder.clone(), |folder| {
    orchester_exec::external::open_path(folder)
  }));
  let editor_button = ActionButton::new(
    "task-delivery-open-editor",
    ButtonKind::Secondary,
    ctx.t("delivery.task.resolve.openEditor"),
  )
  .icon(Icon::CodeXml);
  let open_editor = match Services::of(ctx).editor {
    Some(editor) => {
      let (folder, files) = (worktree.to_path_buf(), conflicted(view));
      editor_button.on_click(move || {
        // Window input from an MCP client starts only an editor a person named for this.
        if is_synthetic_input() && !editor.named() {
          tracing::warn!("synthetic input asked to open an editor no person named; not opened");
          return;
        }
        let (editor, files) = (editor.clone(), files.clone());
        opened("open-worktree-editor", folder.clone(), move |folder| editor.open(folder, &files));
      })
    }
    None => editor_button.unavailable(ctx.t("decisions.ready.resolve.noEditor")),
  };
  let back = back.clone();
  let back_button = ActionButton::new(
    "task-delivery-back",
    ButtonKind::Ghost,
    ctx.t("delivery.task.resolve.backToDecision"),
  )
  .icon(Icon::ArrowLeft)
  .on_click(move || back.set(true));
  let resolved = view.acts.button(
    ActionButton::new(
      "task-delivery-resolved",
      ButtonKind::Primary,
      ctx.t("delivery.task.resolve.resolved"),
    )
    .icon(Icon::Check),
    Act::Resolved,
  );
  let refused = view.acts.refused().map(|refused| {
    crate::ui::field::field_error(
      "task-delivery-refused",
      &crate::ui::message::message_text(ctx, &refused),
    )
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Row::new()
        .id("task-delivery-resolve-actions")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(open_folder)
        .child(open_editor)
        .child(Spacer::new().flex(1.0))
        .child(back_button)
        .child(resolved),
    )
    .with_children(refused)
    .into()
}

/// The files that conflicted, as the queue recorded them.
fn conflicted(view: &View<'_>) -> Vec<String> {
  match view.queue.delivery.as_ref().map(|delivery| &delivery.entry.state) {
    Some(orchester_core::merge_queue::QueueState::Left(orchester_core::merge_queue::Left::Conflicts { files })) => {
      files.clone()
    }
    _ => Vec::new(),
  }
}

/// Runs `open` on `folder` on a thread of its own, named `name`.
fn opened(name: &str, folder: PathBuf, open: impl FnOnce(&Path) -> std::io::Result<()> + Send + 'static) {
  let started = std::thread::Builder::new().name(name.to_owned()).spawn(move || {
    if let Err(error) = open(&folder) {
      tracing::warn!(%error, "the run's worktree did not open");
    }
  });
  if let Err(error) = started {
    tracing::error!(%error, "cannot start opening the run's worktree");
  }
}
