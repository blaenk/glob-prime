use regex::Regex;
use std::fmt;

use self::Token::{
  Char,
  AnyChar,
  AnySequence,
  AnyRecursiveSequence,
  AnyWithin,
  AnyExcept
};
use self::CharSpecifier::{SingleChar, CharRange};

#[deriving(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Show)]
enum Token {
  Char(char),
  AnyChar,
  AnySequence,
  AnyRecursiveSequence,
  AnyWithin(Vec<CharSpecifier>),
  AnyExcept(Vec<CharSpecifier>)
}

#[deriving(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Show)]
enum CharSpecifier {
  SingleChar(char),
  CharRange(char, char)
}

pub struct Pattern {
  re: Regex,
}

pub struct Error {
  pub pos: uint,
  pub msg: String,
}

impl fmt::Show for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Pattern syntax error near position {}: {}",
           self.pos, self.msg)
  }
}

impl Pattern {
  pub fn new(pattern: &str) -> Result<Pattern, Error> {
    Pattern::parse(pattern).and_then(|tokens| Pattern::compile(tokens))
  }

  fn parse(pattern: &str) -> Result<Vec<Token>, Error> {
    let chars = pattern.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
      match chars[i] {
        '?' => {
          tokens.push(AnyChar);
          i += 1;
        }
        '*' => {
          let old = i;

          while i < chars.len() && chars[i] == '*' {
            i += 1;
          }

          let count = i - old;

          if count > 2 {
            return Err(
              Error {
                pos: old + 2,
                msg: "wildcards are either regular `*` or recursive `**`".to_string(),
              })
          }

          else if count == 2 {
            // ** can only be an entire path component
            // i.e. a/**/b is valid, but a**/b or a/**b is not
            // invalid matches are treated literally
            let is_valid =
              // begins with '/' or is the beginning of the pattern
              if i == 2 || chars[i - count - 1] == '/' {
                // it ends in a '/'
                if i < chars.len() && chars[i] == '/' {
                  i += 1;
                  true
                  // or the pattern ends here
                } else if i == chars.len() {
                  true
                  // `**` ends in non-separator
                } else {
                    return Err(
                      Error  {
                        pos: i,
                        msg: concat!(
                          "recursive wildcards `**` must form ",
                          "a single path component, e.g. a/**/b").to_string(),
                        });
                }
                // `**~ begins with non-separator
              } else {
                return Err(
                  Error  {
                    pos: old - 1,
                    msg: concat!(
                      "recursive wildcards `**` must form ",
                      "a single path component, e.g. a/**/b").to_string(),
                    });
              };

            let tokens_len = tokens.len();

            if is_valid {
              // collapse consecutive AnyRecursiveSequence to a single one
              if !(tokens_len > 1 && tokens[tokens_len - 1] == AnyRecursiveSequence) {
                tokens.push(AnyRecursiveSequence);
              }
            }
          } else {
            tokens.push(AnySequence);
          }
        }
        '[' => {
          if i <= chars.len() - 4 && chars[i + 1] == '!' {
            match chars.slice_from(i + 3).position_elem(&']') {
              None => (),
              Some(j) => {
                let chars = chars.slice(i + 2, i + 3 + j);
                let cs = Pattern::parse_character_class(chars);
                tokens.push(AnyExcept(cs));
                i += j + 4;
                continue;
              }
            }
          }

          else if i <= chars.len() - 3 && chars[i + 1] != '!' {
            match chars.slice_from(i + 2).position_elem(&']') {
              None => (),
              Some(j) => {
                let cs = Pattern::parse_character_class(chars.slice(i + 1, i + 2 + j));
                tokens.push(AnyWithin(cs));
                i += j + 3;
                continue;
              }
            }
          }

          // if we get here then this is not a valid range pattern
          tokens.push(Char('['));
          i += 1;
        }
        c => {
          tokens.push(Char(c));
          i += 1;
        }
      }
    }

    Ok(tokens)
  }

  fn parse_character_class(s: &[char]) -> Vec<CharSpecifier> {
    let mut cs = Vec::new();
    let mut i = 0;

    while i < s.len() {
      if i + 3 <= s.len() && s[i + 1] == '-' {
        cs.push(CharRange(s[i], s[i + 2]));
        i += 3;
      } else {
        cs.push(SingleChar(s[i]));
        i += 1;
      }
    }

    return cs;
  }

  fn emit_set(pattern: &mut String, specs: &Vec<CharSpecifier>) {
    for &spec in specs.iter() {
      match spec {
        SingleChar(c) => pattern.push(c),
        CharRange(a, b) =>
          pattern.push_str(
            format!("{start}-{end}", start = a, end = b).as_slice()),
      }
    }
  }

  fn escape_regex_char(c: char) -> String {
    let mut escaped = String::new();
    match c {
      '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' |
        '[' | ']' | '{' | '}' | '^' | '$' => {
          escaped.push('\\');
        },
        _ => (),
    }

    escaped.push(c);
    return escaped;
  }

  fn compile(tokens: Vec<Token>) -> Result<Pattern, Error> {
    let mut re = String::new();

    for token in tokens.iter() {
      match *token {
        Char(c) => re.push_str(Pattern::escape_regex_char(c).as_slice()),
        AnyChar => re.push('.'),
        AnySequence =>
          re.push_str(
            format!(r"[^{sep}]*",
                    sep = Pattern::escape_regex_char(::std::path::SEP).as_slice()).as_slice()),
        AnyRecursiveSequence => re.push_str(".*"),
        AnyWithin(ref specs) => {
          re.push('[');
          Pattern::emit_set(&mut re, specs);
          re.push(']');
        },
        AnyExcept(ref specs) => {
          re.push_str("[^");
          Pattern::emit_set(&mut re, specs);
          re.push(']');
        }
      }
    }

    re.push_str(r"\z(?ms)");

    let compiled = Regex::new(re.as_slice());

    compiled
      .map(|r| Pattern { re: r })
      .map_err(|e| Error { pos: e.pos, msg: e.msg })
  }
}

impl fmt::Show for Pattern {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.re)
  }
}

#[cfg(test)]
mod test {
  use super::Pattern;

  #[test]
  fn translation() {
    let pat = Pattern::new("some/**/te*t.t?t").unwrap().to_string();
    assert!(pat == r"some/.*te[^/]*t\.t.t\z(?ms)");

    let pat = Pattern::new("some/*/te*t.t?t").unwrap().to_string();
    assert!(pat == r"some/[^/]*/te[^/]*t\.t.t\z(?ms)");

    let pat = Pattern::new("one/**").unwrap().to_string();
    assert!(pat == r"one/.*\z(?ms)");
  }

  #[test]
  fn errors() {
    let err = Pattern::new("a/**b").unwrap_err();
    assert!(err.pos == 4);

    let err = Pattern::new("a/bc**").unwrap_err();
    assert!(err.pos == 3);

    let err = Pattern::new("a/*****").unwrap_err();
    assert!(err.pos == 4);

    let err = Pattern::new("a/b**c**d").unwrap_err();
    assert!(err.pos == 2);
  }

  #[test]
  fn classes() {
    let pat = Pattern::new("cache/[abc]/files").unwrap();
    assert!(pat.re.is_match("cache/a/files"));
    assert!(pat.re.is_match("cache/b/files"));
    assert!(pat.re.is_match("cache/c/files"));

    let pat = Pattern::new("cache/[][!]/files").unwrap();
    assert!(pat.re.is_match("cache/[/files"));
    assert!(pat.re.is_match("cache/]/files"));
    assert!(pat.re.is_match("cache/!/files"));
    assert!(!pat.re.is_match("cache/a/files"));

    let pat = Pattern::new(r"cache/[[?*]/files").unwrap();
    assert!(pat.re.is_match("cache/[/files"));
    assert!(pat.re.is_match("cache/?/files"));
    assert!(pat.re.is_match("cache/*/files"));
  }

  #[test]
  fn ranges() {
    let pat = Pattern::new("cache/[A-Fa-f0-9]/files").unwrap();
    assert!(pat.re.is_match("cache/B/files"));
    assert!(pat.re.is_match("cache/b/files"));
    assert!(pat.re.is_match("cache/7/files"));

    let pat = Pattern::new("cache/[!A-Fa-f0-9]/files").unwrap();
    assert!(!pat.re.is_match("cache/B/files"));
    assert!(!pat.re.is_match("cache/b/files"));
    assert!(!pat.re.is_match("cache/7/files"));

    let pat = Pattern::new("cache/[]-]/files").unwrap();
    assert!(pat.re.is_match("cache/]/files"));
    assert!(pat.re.is_match("cache/-/files"));
    assert!(!pat.re.is_match("cache/0/files"));
  }
}
