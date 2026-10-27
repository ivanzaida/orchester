//! An answer that shows an image (`read_file` on Claude Code): its MCP result holds the text block, then an image
//! block with the PNG as base64; redaction changes the text and never the image's data.

use std::sync::Arc;

use orchester_core::{
  command::SecretVariable,
  orchester_tools::{AnswerImage, ToolResult},
  secret::SecretValue,
};
use serde_json::json;

#[test]
fn image_answer_is_a_text_block_then_a_png_image_block() {
  let answer = ToolResult::with_image(
    "PNG read, 2882×1875, sent at 1568×1020".to_owned(),
    AnswerImage::png(b"png"),
  );
  assert!(!answer.is_error());
  assert_eq!(answer.text(), "PNG read, 2882×1875, sent at 1568×1020");
  assert_eq!(
    answer.mcp(),
    json!({"content": [
      {"type": "text", "text": "PNG read, 2882×1875, sent at 1568×1020"},
      {"type": "image", "data": "cG5n", "mimeType": "image/png"}
    ], "isError": false})
  );
  // A log of the answer names the image's size, not its data.
  assert!(!format!("{answer:?}").contains("cG5n"));
  assert_eq!(
    ToolResult::shown("text".to_owned()).mcp()["content"]
      .as_array()
      .map(Vec::len),
    Some(1)
  );
}

#[test]
fn image_answer_redaction_leaves_the_image_data_intact() {
  // Thirty zero bytes are forty `A`s as base64, which a secret `AAAA` matches.
  let image = AnswerImage::png(&[0; 30]);
  let data = image.data().to_owned();
  assert_eq!(data, "A".repeat(40));
  let secret = SecretVariable::new("short", Arc::new(SecretValue::new("AAAA".to_owned()).expect("secret")));
  let answer = ToolResult::with_image("PNG read, AAAA".to_owned(), image).redacted(&[secret]);
  assert!(!answer.text().contains("AAAA"), "{}", answer.text());
  assert_eq!(answer.image().map(AnswerImage::data), Some(data.as_str()));
  assert_eq!(answer.mcp()["content"][1]["data"], json!(data));
}
