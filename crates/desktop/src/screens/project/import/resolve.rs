//! Resolve dependencies (06.05 `QqnDd`, 06.06 `hF664`; body `X03Af`): for the selected resources
//! and everything they refer to, how each is resolved in this project, then Import resources. The
//! copies become local drafts; connections and secrets are never copied.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  import::{Content, Entity, Item, Plan, Role},
  time::Timestamp,
};

use super::{
  BUTTON_HEIGHT, ImportAction, ImportDialog, Stage, beside_gutter, body, footer_bar, labels,
  mapping_row::{MappingChoice, MappingRow, MappingRowProps, RESOLUTION_WIDTH, SOURCE_WIDTH, row_key},
};
use crate::{
  activity::operations,
  import::data::Catalogs,
  theme::{ExtraTypography, gutter_scrollbar},
  ui::{button::ActionButton, field::field_error, icon::Icon, message_text, notice::Notice},
  workspace::OpenProject,
};

/// Height of the column headings (`WcIvG`).
const HEADINGS_HEIGHT: f32 = 30.0;

impl ImportDialog {
  pub(super) fn resolve_body(&self, ctx: &mut Ctx, catalogs: &Catalogs, plan: &Plan) -> Element {
    let rows: Vec<Element> = plan
      .items()
      .iter()
      .map(|item| self.mapping(ctx, catalogs, item))
      .collect();
    let mut table = Column::new().width(Dimension::Pct(100.0)).with_children(rows);
    if copies_tool_access(catalogs, plan) {
      table = table.child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_top(SpacingSize::Md)
          .child(
            Notice::new(
              Icon::ShieldCheck,
              ctx.t("projectImport.access.title"),
              ctx.t("projectImport.access.detail"),
            )
            .id("import-access"),
          ),
      );
    }
    table = table.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Md)
        .child(
          Text::new(&labels::totals(ctx, plan))
            .variant(ExtraTypography::Footnote.role())
            .id("import-totals"),
        ),
    );
    body()
      .spacing(SpacingSize::Md)
      .child(beside_gutter(
        Text::new(&context(ctx, catalogs, plan))
          .variant(TypographyStyle::FieldLabel)
          .into(),
      ))
      .child(
        // The headings sit on the table (`mAnma`); only the rows scroll under them.
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(beside_gutter(headings(ctx, catalogs)))
          .child(
            ScrollVertical::new(table)
              .scrollbar(gutter_scrollbar())
              .width(Dimension::Pct(100.0))
              .flex(1.0),
          ),
      )
      .into()
  }

  fn mapping(&self, ctx: &mut Ctx, catalogs: &Catalogs, item: &Item) -> Element {
    let entity = item.source();
    let rename = self.rename_text(catalogs, entity);
    let (status, status_detail) = labels::status(ctx, catalogs, item);
    let props = MappingRowProps {
      entity,
      name: labels::name(&catalogs.source, entity),
      detail: labels::source_detail(ctx, &catalogs.source, item),
      options: item
        .options()
        .iter()
        .map(|choice| {
          (
            MappingChoice(*choice),
            labels::option(ctx, catalogs, item, *choice, &rename),
          )
        })
        .collect(),
      choice: item.choice(),
      rename,
      status,
      status_detail,
      problem: item.problem().is_some(),
      choices: self.choices.clone(),
    };
    ctx.mount_keyed::<MappingRow>(&format!("import-mapping-{}", row_key(entity)), props)
  }

  /// The name typed for a renamed copy of `entity`, or its own name before anything was typed.
  fn rename_text(&self, catalogs: &Catalogs, entity: Entity) -> String {
    let typed = self.choices.get().0.name(entity).map(str::to_owned);
    typed.unwrap_or_else(|| labels::name(&catalogs.source, entity).to_string())
  }

  /// Back, what keeps the import from starting or what it creates, and Import resources (`SxdEQ`).
  pub(super) fn resolve_footer(
    &self,
    ctx: &mut Ctx,
    project: &OpenProject,
    catalogs: &Catalogs,
    plan: &Plan,
    action: &ImportAction,
  ) -> Element {
    let state = action.state().get();
    let pending = state.is_pending();
    let batch = plan
      .is_ready()
      .then(|| plan.batch(&catalogs.source, self.key, Timestamp::now()));
    if let Some(Err(error)) = &batch {
      tracing::error!(%error, "cannot prepare the copies");
    }
    let note: Element = match (&state.error, &batch) {
      (Some(error), _) if !pending => field_error(&message_text(ctx, error)),
      (_, Some(Err(_))) => field_error(&ctx.t("projectImport.errors.invalidCopy")),
      _ => Text::new(&labels::readiness(ctx, catalogs, plan))
        .variant(TypographyStyle::Caption)
        .id("import-readiness")
        .into(),
    };
    let stage = self.stage.clone();
    let back = ActionButton::new(ctx.t("common.back"))
      .icon(Icon::ArrowLeft)
      .id("import-back")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(move || stage.set(Stage::Select))
      .enabled(!pending)
      .render(ctx);
    let label = if pending {
      ctx.t("projectImport.importing")
    } else {
      ctx.t("projectImport.import")
    };
    let import = ActionButton::new(label)
      .primary()
      .icon(Icon::Download)
      .id("import-submit")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT));
    let import = match batch {
      Some(Ok(batch)) if !pending => {
        let spec =
          operations::import_resources(project, catalogs.source.project().name().as_str(), batch.copies().len());
        let (action, repo_root) = (action.clone(), project.repo_root.clone());
        import.on_click(move || action.run((repo_root.clone(), batch.clone(), spec.clone())))
      }
      _ => import.on_click(|| {}).enabled(false),
    };
    footer_bar(vec![
      back,
      Row::new().flex(1.0).align_items(Alignment::Center).child(note).into(),
      import.render(ctx),
    ])
  }
}

/// "Personal website → Kontur · Preview release v3" (`yQUvB`): the projects and what was selected.
fn context(ctx: &Ctx, catalogs: &Catalogs, plan: &Plan) -> Arc<str> {
  let selected: Vec<&Item> = plan
    .items()
    .iter()
    .filter(|item| item.role() == Role::Selected)
    .collect();
  let (source, target) = (
    catalogs.source.project().name().as_str(),
    catalogs.target.project().name().as_str(),
  );
  match selected.as_slice() {
    [item] => {
      let name = labels::name(&catalogs.source, item.source());
      let version = item
        .pick()
        .map_or_else(|| Arc::from(""), |pick| labels::pick(ctx, pick));
      ctx.t_args(
        "projectImport.context.one",
        [
          ("source", source),
          ("target", target),
          ("name", &*name),
          ("version", &*version),
        ],
      )
    }
    several => ctx.t_args(
      "projectImport.context.other",
      [
        ("source", source),
        ("target", target),
        ("count", several.len().to_string().as_str()),
      ],
    ),
  }
}

/// SOURCE / VERSION, TARGET IN <PROJECT>, RESOLUTION (`WcIvG`).
fn headings(ctx: &Ctx, catalogs: &Catalogs) -> Element {
  let target = catalogs.target.project().name().as_str().to_uppercase();
  let heading = |text: &str| Text::new(text).variant(ExtraTypography::ColumnHeading.role()).nowrap();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADINGS_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Px(SOURCE_WIDTH))
        .child(heading(&ctx.t("projectImport.columns.source"))),
    )
    .child(Column::new().flex(1.0).child(heading(
      &ctx.t_args("projectImport.columns.target", [("project", target.as_str())]),
    )))
    .child(
      Column::new()
        .width(Dimension::Px(RESOLUTION_WIDTH))
        .child(heading(&ctx.t("projectImport.columns.resolution"))),
    )
    .into()
}

/// Whether a copied tool has callers: they are narrowed to the pipelines copied with it.
fn copies_tool_access(catalogs: &Catalogs, plan: &Plan) -> bool {
  plan.items().iter().filter(|item| item.copies()).any(|item| {
    let content = match (item.source(), item.pick()) {
      (Entity::Resource(id), Some(pick)) => catalogs.source.resource(id).and_then(|entry| entry.content(pick)),
      _ => None,
    };
    matches!(content, Some(Content::Tool(tool)) if tool.access().is_some_and(|access| !access.callers().is_empty()))
  })
}
