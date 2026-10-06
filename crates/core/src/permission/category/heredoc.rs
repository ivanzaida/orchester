//! Here-documents (`cat > notes.md <<'EOF'` … `EOF`): the lines after the command, up to the line that
//! is the delimiter, are what the command reads from its standard input, data rather than commands. With
//! a quoted delimiter (`'EOF'`, `"EOF"`, `\EOF`) bash takes the lines as written; with a bare one it
//! expands `$…` and backticks in them, which may run commands, so such a body with `$` or a backtick is not
//! read. `<<-` drops the leading tabs of each line, the delimiter's too. A body that does not end with its
//! delimiter is not read either.

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// A here-document whose body follows the line.
#[derive(Clone, Debug)]
pub(super) struct Heredoc {
  delimiter: String,
  quoted: bool,
  strip_tabs: bool,
}

impl Heredoc {
  /// The here-document whose operator (`<<` or `<<-`) was just taken; `None` when its delimiter cannot be
  /// read.
  pub(super) fn after_operator(chars: &mut Chars<'_>) -> Option<Self> {
    let strip_tabs = chars.next_if_eq(&'-').is_some();
    while chars.next_if(|c| matches!(c, ' ' | '\t')).is_some() {}
    let mut delimiter = String::new();
    let mut quoted = false;
    while let Some(&c) = chars.peek() {
      if c.is_whitespace() || matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')') {
        break;
      }
      chars.next();
      match c {
        '\'' | '"' => {
          quoted = true;
          loop {
            match chars.next()? {
              end if end == c => break,
              inner => delimiter.push(inner),
            }
          }
        }
        '\\' => {
          quoted = true;
          delimiter.push(chars.next()?);
        }
        other => delimiter.push(other),
      }
    }
    (!delimiter.is_empty()).then_some(Self {
      delimiter,
      quoted,
      strip_tabs,
    })
  }

  /// Takes the body from `chars`, which start at its first line; whether it is data: it ends with the
  /// delimiter and, with a bare delimiter, expands nothing.
  pub(super) fn take_body(&self, chars: &mut Chars<'_>) -> bool {
    let mut data = true;
    loop {
      if chars.peek().is_none() {
        return false;
      }
      let mut line = String::new();
      while let Some(c) = chars.next_if(|c| *c != '\n') {
        line.push(c);
      }
      chars.next_if_eq(&'\n');
      let line = if self.strip_tabs {
        line.trim_start_matches('\t')
      } else {
        line.as_str()
      };
      if line == self.delimiter {
        return data;
      }
      data &= self.quoted || !line.contains(['$', '`']);
    }
  }
}
