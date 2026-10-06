//! Listing processes, which reads and changes nothing: `tasklist` with its filters and formats, `ps`
//! without the forms that print processes' environments, and a PowerShell pipeline that only gets
//! processes and shapes the output (`Get-Process -Id 1 | Select-Object Id,ProcessName`). Their arguments
//! name no file. Everything else, `taskkill` and `Stop-Process` included, is read as before.

use super::programs::{Judged, Kind};

/// The verdict for `tasklist` and `ps`; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  let reads = match program {
    "tasklist" => tasklist(args),
    "ps" => ps(args),
    _ => return None,
  };
  Some(if reads {
    Judged::of(Kind::ReadOnly).known_safe().naming_no_file()
  } else {
    Judged::unknown()
  })
}

/// `tasklist` with only `/FI <filter>`, `/FO <format>`, `/V` and `/NH`, also written `//FI` as Git
/// Bash passes them; `/S` (another machine), `/U`, `/P` and the rest are refused.
fn tasklist(args: &[String]) -> bool {
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    let option = arg.trim_start_matches('/').to_ascii_uppercase();
    if !arg.starts_with('/') {
      return false;
    }
    match option.as_str() {
      "FI" | "FO" => {
        if args.next().is_none() {
          return false;
        }
      }
      "V" | "NH" => {}
      _ => return false,
    }
  }
  true
}

/// `ps` with options that select processes and columns; not `e` or `-E`, which print each process's
/// environment, tokens included.
fn ps(args: &[String]) -> bool {
  const FLAGS: &str = "efAaxlwWj";
  const VALUED: [&str; 4] = ["-p", "-o", "-u", "-U"];
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    if VALUED.contains(&arg.as_str()) {
      if args.next().is_none() {
        return false;
      }
    } else if let Some(flags) = arg.strip_prefix('-') {
      if flags.is_empty() || !flags.chars().all(|flag| FLAGS.contains(flag)) {
        return false;
      }
    } else if !matches!(arg.as_str(), "aux" | "ax" | "axu" | "ux") {
      return false;
    }
  }
  true
}

/// Commands that start a pipeline listing processes.
const SOURCES: [&str; 3] = ["get-process", "get-ciminstance", "get-wmiobject"];
/// Commands a listing may be piped into.
const SHAPES: [&str; 6] = [
  "select-object",
  "format-table",
  "format-list",
  "sort-object",
  "out-string",
  "where-object",
];
/// Parameters of the sources, and the class the CIM and WMI ones may get.
const SOURCE_PARAMETERS: [&str; 6] = ["-id", "-name", "-filter", "-erroraction", "-classname", "-class"];
const PROCESS_CLASS: &str = "win32_process";
/// Parameters of the shaping commands that take a value or none.
const SHAPE_PARAMETERS: [&str; 9] = [
  "-property",
  "-first",
  "-last",
  "-expandproperty",
  "-autosize",
  "-wrap",
  "-descending",
  "-width",
  "-unique",
];
/// Comparisons of a `Where-Object` filter.
const COMPARISONS: [&str; 9] = ["-eq", "-ne", "-like", "-notlike", "-match", "-gt", "-lt", "-ge", "-le"];

/// Whether `script` (what `pwsh -Command` runs) is only a process listing: a source with its parameters,
/// then shaping commands, joined by `|`, with no other character that could run or write anything.
pub(super) fn listing(script: &str) -> bool {
  let Some(segments) = segments(script) else {
    return false;
  };
  let Some((source, shapes)) = segments.split_first() else {
    return false;
  };
  is_source(source) && shapes.iter().all(|shape| is_shape(shape))
}

/// The words of each `|`-joined command of `script`, quotes taken away; `None` for a character that
/// could do more than name things (`;`, `&`, `$(`, `@`, `` ` ``, `>`, `<`, a new line, `$` in a double
/// quote) or a quote that is not closed.
fn segments(script: &str) -> Option<Vec<Vec<String>>> {
  let mut segments = vec![Vec::new()];
  let mut word: Option<String> = None;
  let mut chars = script.chars();
  while let Some(c) = chars.next() {
    match c {
      ' ' | '\t' => end(&mut segments, &mut word),
      '|' => {
        end(&mut segments, &mut word);
        segments.push(Vec::new());
      }
      '\'' | '"' => {
        let quoted = word.get_or_insert_with(String::new);
        loop {
          match chars.next()? {
            close if close == c => break,
            '$' | '`' if c == '"' => return None,
            inner => quoted.push(inner),
          }
        }
      }
      ';' | '&' | '@' | '`' | '>' | '<' | '\n' | '\r' | '(' | ')' | '[' | ']' => return None,
      other => word.get_or_insert_with(String::new).push(other),
    }
  }
  end(&mut segments, &mut word);
  segments.iter().all(|segment| !segment.is_empty()).then_some(segments)
}

fn end(segments: &mut [Vec<String>], word: &mut Option<String>) {
  if let (Some(word), Some(segment)) = (word.take(), segments.last_mut()) {
    segment.push(word);
  }
}

/// `Get-Process` with `-Id`, `-Name`, `-ErrorAction`, or `Get-CimInstance`/`Get-WmiObject` of
/// `Win32_Process` with `-Filter` and `-ErrorAction`.
fn is_source(words: &[String]) -> bool {
  let Some((command, rest)) = words.split_first() else {
    return false;
  };
  let command = command.to_lowercase();
  if !SOURCES.contains(&command.as_str()) || words.iter().any(|word| word.contains('$') || word.contains('{')) {
    return false;
  }
  let process = command == "get-process";
  let mut class = process;
  let mut words = rest.iter();
  while let Some(word) = words.next() {
    let lower = word.to_lowercase();
    if lower == PROCESS_CLASS {
      class = true;
    } else if SOURCE_PARAMETERS.contains(&lower.as_str()) {
      let Some(value) = words.next() else {
        return false;
      };
      class |= matches!(lower.as_str(), "-classname" | "-class") && value.eq_ignore_ascii_case(PROCESS_CLASS);
    } else if !process || word.starts_with('-') {
      return false;
    }
  }
  class
}

/// A shaping command with property names and its listed parameters, or `Where-Object` with one
/// comparison, written `Prop -eq Value` or `{ $_.Prop -eq Value }`.
fn is_shape(words: &[String]) -> bool {
  let Some((command, rest)) = words.split_first() else {
    return false;
  };
  let command = command.to_lowercase();
  if !SHAPES.contains(&command.as_str()) {
    return false;
  }
  if command == "where-object" {
    return is_comparison(rest);
  }
  rest.iter().all(|word| {
    let lower = word.to_lowercase();
    SHAPE_PARAMETERS.contains(&lower.as_str()) || (!word.starts_with('-') && is_names(word))
  })
}

/// `Prop -op Value` or `{ $_.Prop -op Value }`.
fn is_comparison(words: &[String]) -> bool {
  let inner: Vec<&str> = match words {
    [open, middle @ .., close] if open == "{" && close == "}" => middle.iter().map(String::as_str).collect(),
    _ => words.iter().map(String::as_str).collect(),
  };
  let [property, comparison, value] = inner.as_slice() else {
    return false;
  };
  let property = property.strip_prefix("$_.").unwrap_or(property);
  is_names(property)
    && !property.contains(',')
    && COMPARISONS.contains(&comparison.to_lowercase().as_str())
    && !value.contains(['$', '{', '}'])
}

/// Property names, a comma between them, a number, or a plain value: letters, digits and `_.,*-`.
fn is_names(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "_.,*-".contains(c))
}
