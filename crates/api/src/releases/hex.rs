//! Lowercase hex, the one spelling the release contract writes and accepts for digests, key ids and signatures.

pub(super) fn encode(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `text` as `N` bytes: exactly `2 * N` lowercase hex digits, so every value has one spelling.
pub(super) fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
  if text.len() != N * 2 || !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
    return None;
  }
  let mut bytes = [0_u8; N];
  for (index, byte) in bytes.iter_mut().enumerate() {
    *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
  }
  Some(bytes)
}
