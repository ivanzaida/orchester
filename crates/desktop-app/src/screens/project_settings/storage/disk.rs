//! Disk group eQIFv: the meter belongs to the disk row; states live in its rows.
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Stack, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::messages::Message;

use super::{StoragePage, TabProps, bytes, error, read::Inventory, retry, total};
use crate::{
  screens::project_settings::kit::{Group, SettingRow, Tone},
  services::storage::Disk,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::NumberField,
    skeleton,
    unavailable::reason_line,
  },
};

/// Disk meter and value box dimensions (PJFOw, Keep free in Jsbd1).
const METER: f32 = 6.0;
const MARK: f32 = 2.0;
const METER_WIDTH: f32 = 240.0;
const FIELD_WIDTH: f32 = 120.0;

pub fn summary(ctx: &Ctx, disk: &Disk) -> Arc<str> {
  let Some(usage) = disk.guard.disk else {
    return ctx.t("settings.project.runs.storage.diskUnavailable");
  };
  ctx.t_args(
    if disk.guard.holds() {
      match disk.paused {
        0 => "settings.project.runs.storage.diskLowWaiting",
        1 => "settings.project.runs.storage.diskLowOne",
        _ => "settings.project.runs.storage.diskLow",
      }
    } else {
      "settings.project.runs.storage.diskSummary"
    },
    [
      ("free", bytes(ctx, Some(usage.free)).to_string()),
      ("total", bytes(ctx, Some(usage.total)).to_string()),
      ("keep", disk.guard.keep_free.gigabytes().to_string()),
      ("count", disk.paused.to_string()),
    ],
  )
}

pub fn group(
  ctx: &mut Ctx,
  page: &StoragePage,
  props: &TabProps,
  disk: Option<&Result<Disk, Message>>,
  inventory: Option<&Inventory>,
) -> Element {
  let hint = match disk {
    Some(Ok(disk)) => summary(ctx, disk),
    Some(Err(e)) => crate::ui::message::message_text(ctx, e),
    None => ctx.t("settings.project.runs.storage.readingDisk"),
  };
  let mut drive = SettingRow::new(
    "storage-disk",
    ctx.t_args(
      "settings.project.runs.storage.diskNamed",
      [("disk", name(&props.project.open.repo_root))],
    ),
  )
  .toned(
    hint,
    if disk.is_some_and(|r| r.as_ref().is_ok_and(|d| d.guard.holds())) {
      Tone::Warning
    } else {
      Tone::Muted
    },
  );
  if let Some(Ok(disk)) = disk {
    drive = drive.control(meter(ctx, disk, inventory.and_then(|r| total(&r.rows))));
  } else {
    drive = drive.control(skeleton::line(ctx, TypographyStyle::Body, METER_WIDTH));
  }
  let keep = keep(
    ctx,
    page,
    disk.is_some_and(Result::is_ok),
    &name(&props.project.open.repo_root),
  );
  let build = SettingRow::new(
    "storage-build-after",
    ctx.t("settings.project.runs.storage.build_output_after.title"),
  )
  .hint(ctx.t("settings.project.runs.storage.build_output_after.hint"))
  .control(
    ActionButton::new(
      "storage-build-after-unavailable",
      ButtonKind::Select,
      ctx.t("settings.project.runs.storage.build_output_after.unavailableValue"),
    )
    .unavailable(ctx.t("settings.project.runs.storage.build_output_after.unavailable")),
  )
  .under(reason_line(
    "storage-build-after-reason",
    &ctx.t("settings.project.runs.storage.build_output_after.unavailable"),
  ))
  .render(ctx);
  let cleanup = cleanup(ctx, page, inventory);
  let mut group = Group::new("storage-disk-group", ctx.t("settings.project.runs.storage.disk"))
    .row(drive.render(ctx))
    .row(keep)
    .row(build)
    .row(cleanup);
  if disk.is_some_and(Result::is_err) {
    group = group.end(retry(ctx, "storage-read-again", &page.refresh));
  }
  group.into()
}

fn keep(ctx: &mut Ctx, page: &StoragePage, loaded: bool, disk: &str) -> Element {
  let mut field = NumberField::new(
    "storage-keep-free",
    ctx.t("settings.project.runs.storage.keep_free.title"),
    page.keep.clone(),
    ctx.t("settings.project.runs.storage.gb"),
    Arc::<str>::from(""),
  )
  .width(Dimension::Px(FIELD_WIDTH))
  .on_leave(page.save_keep());
  if !loaded || page.save.state().get().is_pending() {
    field = field.locked();
  }
  if page.invalid.get() {
    field = field.error(ctx.t("settings.project.runs.storage.keep_free.invalid"));
  }
  let save = page.save_keep();
  let modal = page.modal.clone();
  let control = Column::new()
    .on_key_down(move |event: lurq::app::events::KeyboardEvent| {
      if event.key == "Enter" && event.text_input_focused && !modal.get_untracked() {
        save();
        event.prevent_default();
      }
    })
    .child(field.render(ctx));
  let mut row = SettingRow::new(
    "storage-keep",
    ctx.t_args("settings.project.runs.storage.keep_free.named", [("disk", disk)]),
  )
  .hint(ctx.t("settings.project.runs.storage.keep_free.hint"))
  .control(control);
  if let Some(e) = page.save.state().get().error {
    row = row.under(error(ctx, "storage-save-error", &e));
  }
  row.render(ctx)
}

fn cleanup(ctx: &mut Ctx, page: &StoragePage, inventory: Option<&Inventory>) -> Element {
  let runs = inventory
    .map(|r| {
      r.rows
        .iter()
        .filter_map(orchester_workspace::storage::KeptRow::removable_run)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  let ready = inventory.is_some_and(|r| r.finished);
  let key = if !ready {
    "settings.project.runs.storage.clean_up.measuring"
  } else if runs.is_empty() {
    "settings.project.runs.storage.clean_up.nothing"
  } else if runs.len() == 1 {
    "settings.project.runs.storage.clean_up.hintOne"
  } else {
    "settings.project.runs.storage.clean_up.hint"
  };
  let size = inventory.and_then(|r| {
    total(
      &r.rows
        .iter()
        .filter(|r| r.removable_run().is_some())
        .cloned()
        .collect::<Vec<_>>(),
    )
  });
  let hint = ctx.t_args(
    key,
    [
      ("count", runs.len().to_string()),
      ("size", bytes(ctx, size).to_string()),
    ],
  );
  let mut button = ActionButton::new(
    "storage-clean-up",
    ButtonKind::Secondary,
    ctx.t("settings.project.runs.storage.clean_up.title"),
  )
  .compact();
  if page.prepare.state().get().is_pending() || page.remove.state().get().is_pending() {
    button = button.loading();
  } else if !ready || runs.is_empty() || page.busy() {
    button = button.disabled();
  } else {
    let ask = page.ask(runs);
    button = button.on_click(move || ask());
  }
  let mut row = SettingRow::new("storage-cleanup", ctx.t("settings.project.runs.storage.clean_up.title"))
    .hint(hint)
    .control(button);
  if let Some(e) = page.prepare.state().get().error.or(page.remove.state().get().error) {
    row = row.under(error(ctx, "storage-clean-error", &e));
  }
  row.render(ctx)
}

fn meter(ctx: &Ctx, disk: &Disk, kept: Option<u64>) -> Element {
  let Some(usage) = disk.guard.disk.filter(|d| d.total > 0) else {
    return Row::new().into();
  };
  let pct = |n: u64| ((n as f64 / usage.total as f64) * 100.0).clamp(0.0, 100.0) as f32;
  let used = usage.total.saturating_sub(usage.free);
  let own = kept.unwrap_or(0).min(used);
  let bar = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(METER))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .clip()
    .child(
      Row::new()
        .width(Dimension::Pct(pct(used - own)))
        .height(Dimension::Pct(100.0))
        .background(PaletteColor::BorderStrong),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(pct(own)))
        .height(Dimension::Pct(100.0))
        .background(PaletteColor::Info),
    );
  let mark = Row::new()
    .width(Dimension::Pct(100.0 - pct(disk.guard.keep_free.bytes())))
    .height(Dimension::Px(METER))
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .width(Dimension::Px(MARK))
        .height(Dimension::Px(METER))
        .background(PaletteColor::Warning),
    );
  Column::new()
    .width(Dimension::Px(METER_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(
      Stack::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(METER))
        .child(bar)
        .child(mark),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t(if kept.is_some() {
            "settings.project.runs.storage.orchester"
          } else {
            "settings.project.runs.storage.orchesterUnknown"
          }))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Info),
        )
        .child(
          Text::new(&ctx.t("settings.project.runs.storage.keep_free.mark"))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::Warning),
        )
        .child(Spacer::new().flex(1.0))
        .with_children(kept.map(|_| Text::new(&bytes(ctx, Some(usage.free))).variant(ExtraTypography::Note.role()))),
    )
    .into()
}

/// The disk's path root; disk usage itself is read at the repository, including mounted volumes.
pub fn name(root: &std::path::Path) -> String {
  let prefix = root
    .components()
    .next()
    .map(|part| part.as_os_str().to_string_lossy().into_owned())
    .unwrap_or_default();
  orchester_workspace::display::plain_path(&prefix)
}
