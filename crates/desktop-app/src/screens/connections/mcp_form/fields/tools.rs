//! *Run without asking* (`Run without asking` `hbex3` in `11 · Add MCP server`): the field and *Add* that add tool
//! names as the server names them (`Entry row` `BJ0WK`), several at once when separated by spaces, commas or lines, and
//! the tools added as removable pills under them (`Pills` `R2fM7K`). The tools are kept one per line, as the server's
//! fields are checked and saved; a name that cannot be used says why under the field.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};

use super::Fields;
use crate::{
  screens::project_settings::kit::{FormField, form_note, on_enter},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, field_error},
    icon::Icon,
  },
};

impl Fields {
  pub(super) fn tools_row(&self, ctx: &mut Ctx, error: Option<Arc<str>>) -> Element {
    let listed = tools(&self.exempt_tools.get());
    let pills = (!listed.is_empty()).then(|| {
      Row::new()
        .id("connection-mcp-tools")
        .width(Dimension::Pct(100.0))
        .wrap()
        .spacing(ExtraSpacing::Snug.role())
        .with_children(listed.iter().enumerate().map(|(index, tool)| {
          let (tool, kept) = (tool.clone(), self.exempt_tools.clone());
          Element::from(
            TagChip::new(format!("connection-mcp-tool-{index}"), tool.as_str())
              .mono()
              .pill()
              .removable(ctx.t("connections.mcp.dialog.removeTool"), move || {
                kept.update(|text| *text = remove(text, &tool));
              }),
          )
        }))
    });
    let entry = FieldInput::new(
      "connection-mcp-exempt-tools",
      ctx.t("connections.mcp.exemptTools"),
      self.tool.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.addToolPlaceholder"))
    .mono()
    .render(ctx);
    let (kept, typed) = (self.exempt_tools.clone(), self.tool.clone());
    let add = ActionButton::new(
      "connection-mcp-exempt-tools-add",
      ButtonKind::Secondary,
      ctx.t("connections.groups.add"),
    )
    .icon(Icon::Plus)
    .in_form()
    .on_click(move || add_typed(&kept, &typed));
    let (kept, typed) = (self.exempt_tools.clone(), self.tool.clone());
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .on_key_down(on_enter(move || add_typed(&kept, &typed)))
          .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(entry))
          .child(add),
      )
      .with_children(error.map(|error| field_error("connection-mcp-exempt-tools-error", &error)))
      .with_children(pills)
      .child(form_note(
        "connection-mcp-exempt-tools-note",
        &ctx.t("connections.mcp.dialog.toolsNote"),
      ));
    FormField::new(
      "connection-mcp-exempt-tools",
      ctx.t("connections.mcp.dialog.runWithoutAsking"),
    )
    .hint(ctx.t("connections.mcp.dialog.runWithoutAskingHint"))
    .info(ctx.t("connections.mcp.dialog.exemptToolsHint"))
    .render(ctx, control)
  }
}

/// The tool names kept, one per line, in order.
fn tools(text: &str) -> Vec<String> {
  text
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

/// `text` without the line `tool`.
fn remove(text: &str, tool: &str) -> String {
  tools(text)
    .into_iter()
    .filter(|kept| kept != tool)
    .collect::<Vec<_>>()
    .join("\n")
}

/// Adds the names typed (separated by spaces, commas or lines) that are not kept yet, and empties the field.
fn add_typed(kept: &Signal<String>, typed: &Signal<String>) {
  let added = typed.get();
  let names = added
    .split(|c: char| c.is_whitespace() || c == ',')
    .filter(|name| !name.is_empty());
  kept.update(|text| {
    let mut listed = tools(text);
    for name in names {
      if !listed.iter().any(|kept| kept == name) {
        listed.push(name.to_owned());
      }
    }
    *text = listed.join("\n");
  });
  typed.set(String::new());
}
