//! Create drafts (`A5Av4`; while importing and when it fails, `TRIh0`; refused, `ApCgj` in `oUQ3O`): what the import
//! creates, reuses and leaves unresolved, and Import resources, which writes the copies at once. Nothing is written
//! before it, and a failure writes nothing: the page keeps every choice.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{import::Plan, time::Timestamp};
use orchester_workspace::import::{
  data::{Catalogs, ImportFailure},
  drafts, review,
};

use super::{State, Step, drafts_view, frame, select::body, words};
use crate::{
  project::ShownProject,
  theme::{IconSize, ImportSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The step's body and its footer.
pub(super) fn step(
  ctx: &mut Ctx,
  state: &State,
  (project, catalogs): (&ShownProject, &Catalogs),
) -> (Element, Element) {
  let plan = state.plan(catalogs);
  let names = (
    catalogs.source.project().name().as_str(),
    catalogs.target.project().name().as_str(),
  );
  let work = state.work.state().get();
  let failure = work
    .error
    .clone()
    .filter(|_| !work.is_pending() && !state.dismissed.get());
  let summary = drafts::drafts(catalogs, &plan);
  let context = words::context(ctx, names, &review::selection(catalogs, &plan));
  let mut content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ImportSpacing::Ready.role())
    .padding_bottom(SpacingSize::Lg);
  if let Some(failure) = &failure {
    content = content.child(failed(ctx, failure, names));
  }
  content = content
    .child(
      Column::new()
        .id("import-ready-head")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&ctx.t("import.ready.title"))
            .id("import-ready-title")
            .variant(TypographyStyle::Title),
        )
        .child(
          Text::new(&ctx.t_args("import.ready.detail", [("context", &*context)]))
            .id("import-ready-detail")
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Description),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xl)
        .align_items(Alignment::Start)
        .child(drafts_view::what_happens(ctx, &summary, names))
        .child(drafts_view::policy_and_access(ctx, &summary.access, names)),
    );
  let body = body(ImportSpacing::Ready.role()).child(
    ScrollVertical::new(content)
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0)),
  );
  let footer = footer(ctx, state, (project, catalogs), (&plan, failure.as_ref()));
  (body.into(), footer)
}

/// Why the copies could not be written (`h4nVTN`, `mv1l1`, `ApCgj`); nothing was.
fn failed(ctx: &Ctx, failure: &ImportFailure, (source, target): (&str, &str)) -> Element {
  match failure {
    ImportFailure::Changed => Notice::new(
      "import-failed",
      NoticeTone::Warning,
      ctx.t("import.failed.changedTitle"),
    )
    .body(ctx.t("import.failed.changedDetail")),
    ImportFailure::Refused { folder } => Notice::new(
      "import-failed",
      NoticeTone::Danger,
      ctx.t_args("import.failed.refusedTitle", [("project", target)]),
    )
    .icon(Icon::Lock)
    .body(ctx.t_args("import.failed.refusedDetail", [("folder", folder.as_str())])),
    ImportFailure::Failed(reason) => Notice::new(
      "import-failed",
      NoticeTone::Danger,
      ctx.t_args("import.failed.title", [("project", source)]),
    )
    .body(ctx.t("import.failed.detail"))
    .part(
      Text::new(&message_text(ctx, reason))
        .id("import-failed-reason")
        .variant(TypographyStyle::Caption),
    ),
  }
  .into()
}

/// The totals, Back, and Import resources: Importing… while it writes, Try again after a failure, and Check the
/// mappings when this project changed meanwhile (`Tadoz`, `cb4dh`, `h4nVTN`, `mv1l1`, `bvrhS`).
fn footer(
  ctx: &mut Ctx,
  state: &State,
  (project, catalogs): (&ShownProject, &Catalogs),
  (plan, failure): (&Plan, Option<&ImportFailure>),
) -> Element {
  let pending = state.work.state().get().is_pending();
  let step = state.step.clone();
  let back = ActionButton::new("import-previous", ButtonKind::Secondary, ctx.t("import.previous"))
    .icon(Icon::ArrowLeft)
    .on_click(move || step.set(Step::Resolve));
  let back = if pending { back.disabled() } else { back };
  let mut end: Vec<Element> = Vec::new();
  let batch = plan.batch(&catalogs.source, state.key, Timestamp::now());
  if let Err(error) = &batch {
    tracing::error!(%error, "cannot prepare the copies");
    end.push(invalid_copy(ctx));
  }
  if !matches!(failure, Some(ImportFailure::Changed)) {
    end.push(back.into());
  }
  let action = match (failure, batch) {
    (Some(ImportFailure::Changed), _) => {
      let (step, revision, dismissed) = (state.step.clone(), state.revision.clone(), state.dismissed.clone());
      ActionButton::new("import-check", ButtonKind::Primary, ctx.t("import.checkMappings"))
        .icon(Icon::ArrowLeft)
        .on_click(move || {
          dismissed.set(true);
          revision.update(|revision| *revision += 1);
          step.set(Step::Resolve);
        })
    }
    (_, Ok(batch)) => {
      let (label, glyph) = if failure.is_some() {
        (ctx.t("import.tryAgain"), Icon::RefreshCw)
      } else if pending {
        (ctx.t("import.importing"), Icon::Download)
      } else {
        (ctx.t("import.import"), Icon::Download)
      };
      let button = ActionButton::new("import-submit", ButtonKind::Primary, label).icon(glyph);
      if pending {
        button.loading()
      } else {
        let (work, dismissed) = (state.work.clone(), state.dismissed.clone());
        let args = (
          project.open.clone(),
          catalogs.source.project().name().as_str().to_owned(),
          batch,
        );
        button.on_click(move || {
          dismissed.set(false);
          work.run(args.clone());
        })
      }
    }
    (_, Err(_)) => ActionButton::new("import-submit", ButtonKind::Primary, ctx.t("import.import"))
      .icon(Icon::Download)
      .disabled(),
  };
  end.push(action.into());
  frame::footer(&words::totals(ctx, plan.counts()), end)
}

/// A copy could not be prepared: its references, resolved here, break a rule of its kind (`bvrhS`).
fn invalid_copy(ctx: &Ctx) -> Element {
  Row::new()
    .id("import-invalid-copy")
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::Danger))
    .child(
      Text::new(&ctx.t("import.failed.invalidCopy"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::Danger)
        .nowrap(),
    )
    .into()
}
