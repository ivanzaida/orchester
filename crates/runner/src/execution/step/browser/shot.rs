//! `browser_screenshot`: a PNG of a tab's visible page, its full page, or one element, saved under `.tmp/browser/` in
//! the worktree and answered with its path relative to the worktree; an agent that is shown images (Claude Code) gets
//! the image too, downscaled and re-encoded as `read_file` sends one.

use std::{
  fs,
  io::Write as _,
  path::{Path, PathBuf},
  time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchester_agent::files::image::{ImageKind, ShownImage};
use orchester_core::orchester_tools::{AnswerImage, browser::ElementRef};
use serde_json::{Value, json};

use super::{Answer, ToolError};
use crate::browser::Tab;

/// Where the screenshots go, relative to the worktree.
const FOLDER: [&str; 2] = [".tmp", "browser"];
/// How long the browser may take to take a screenshot: a full page takes longer than a command.
const SHOT_LIMIT: Duration = Duration::from_secs(60);

/// Takes the screenshot of `tab`: `element` only, the full page when `full_page`, else what is in view. Saved in
/// `worktree`; the image comes back as well when `images`.
pub(super) async fn take(
  tab: &Tab,
  element: Option<&ElementRef>,
  full_page: bool,
  (worktree, images): (&Path, bool),
) -> Result<Answer, ToolError> {
  let mut params = json!({ "format": "png" });
  if let Some(element) = element {
    params["clip"] = element_clip(tab, element).await?;
    params["captureBeyondViewport"] = Value::Bool(true);
  } else if full_page {
    let metrics = tab.call("Page.getLayoutMetrics", json!({})).await?;
    let size = &metrics["cssContentSize"];
    params["clip"] = json!({ "x": 0, "y": 0, "width": size["width"], "height": size["height"], "scale": 1 });
    params["captureBeyondViewport"] = Value::Bool(true);
  }
  let shot = tab.call_within("Page.captureScreenshot", params, SHOT_LIMIT).await?;
  let png = shot["data"]
    .as_str()
    .and_then(|data| STANDARD.decode(data).ok())
    .ok_or_else(|| ToolError::Page("the browser's screenshot is not a PNG".to_owned()))?;
  let saved = save(worktree, &png)?;
  let what = match (element, full_page) {
    (Some(element), _) => format!("element {}", element.as_str()),
    (None, true) => "full page".to_owned(),
    (None, false) => "visible page".to_owned(),
  };
  let text = format!("PNG of the {what} of tab {} saved: {saved}", tab.id());
  if !images {
    return Ok(Answer::from(format!("{text}. Open it with read_file to see it.")));
  }
  Ok(match ShownImage::of(&png, ImageKind::Png) {
    Ok(shown) => Answer {
      text: format!("{text} ({})", shown.line()),
      image: Some(AnswerImage::png(shown.png())),
    },
    Err(error) => Answer::from(format!("{text}. It is not shown here: {error}")),
  })
}

/// The page's area of `element`, scrolled into view, in CSS pixels of the page.
async fn element_clip(tab: &Tab, element: &ElementRef) -> Result<Value, ToolError> {
  let node = tab.node(element)?;
  tab
    .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": node }))
    .await?;
  let model = tab.call("DOM.getBoxModel", json!({ "backendNodeId": node })).await?;
  let border: Vec<f64> = model["model"]["border"]
    .as_array()
    .map(|points| points.iter().filter_map(Value::as_f64).collect())
    .unwrap_or_default();
  let xs = || border.iter().step_by(2).copied();
  let ys = || border.iter().skip(1).step_by(2).copied();
  let (left, top) = (xs().fold(f64::INFINITY, f64::min), ys().fold(f64::INFINITY, f64::min));
  let (width, height) = (
    xs().fold(f64::NEG_INFINITY, f64::max) - left,
    ys().fold(f64::NEG_INFINITY, f64::max) - top,
  );
  if border.len() != 8 || width < 1.0 || height < 1.0 {
    return Err(ToolError::Page(format!(
      "{} has no box on the page: it is hidden or has no size",
      element.as_str()
    )));
  }
  let metrics = tab.call("Page.getLayoutMetrics", json!({})).await?;
  let viewport = &metrics["cssVisualViewport"];
  let (page_x, page_y) = (
    viewport["pageX"].as_f64().unwrap_or_default(),
    viewport["pageY"].as_f64().unwrap_or_default(),
  );
  Ok(json!({ "x": left + page_x, "y": top + page_y, "width": width, "height": height, "scale": 1 }))
}

/// Saves `png` under `.tmp/browser/` in `worktree`, made when it is missing; its path relative to the worktree, with
/// `/`. Refused when a link leads the folder out of the worktree.
fn save(worktree: &Path, png: &[u8]) -> Result<String, ToolError> {
  let failed = |error: std::io::Error| ToolError::Save(error.to_string());
  let folder: PathBuf = FOLDER.iter().fold(worktree.to_path_buf(), |path, part| path.join(part));
  fs::create_dir_all(&folder).map_err(failed)?;
  let inside = fs::canonicalize(&folder).map_err(failed)?;
  if !inside.starts_with(fs::canonicalize(worktree).map_err(failed)?) {
    return Err(ToolError::Save("a link leads it out of the worktree".to_owned()));
  }
  let mut file = tempfile::Builder::new()
    .prefix("screenshot-")
    .suffix(".png")
    .tempfile_in(&folder)
    .map_err(failed)?;
  file.write_all(png).map_err(failed)?;
  let (_, path) = file.keep().map_err(|error| failed(error.error))?;
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| ToolError::Save("the file's name is not Unicode".to_owned()))?;
  Ok(format!("{}/{name}", FOLDER.join("/")))
}
