//! Prints each of its arguments on a line of its own, as a Rust string literal, so a test can
//! compare exactly what the program received, empty arguments and line breaks included.

use std::io::{self, Write};

fn main() -> io::Result<()> {
  let mut stdout = io::stdout().lock();
  for arg in std::env::args_os().skip(1) {
    writeln!(stdout, "{:?}", arg.to_string_lossy())?;
  }
  Ok(())
}
