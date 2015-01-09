use regex::Regex;
use std::fmt;
use std::path;

use self::Token::{
  Char,
  AnyChar,
  AnySequence,
  AnyRecursiveSequence,
  AnyWithin,
  AnyExcept
};
use self::CharSpecifier::{SingleChar, CharRange};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Show)]
enum Token {
  Char(char),
  AnyChar,
  AnySequence,
  AnyRecursiveSequence,
  AnyWithin(Vec<CharSpecifier>),
  AnyExcept(Vec<CharSpecifier>)
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Show)]
enum CharSpecifier {
  SingleChar(char),
  CharRange(char, char)
}

// TODO: add original string here?
pub struct Pattern {
  re: Regex,
  original: String,
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
    Pattern::parse(pattern).and_then(Pattern::compile)
      .map(|r| Pattern { original: pattern.to_string(), re: r })
  }

  pub fn as_str<'a>(&'a self) -> &'a str {
    self.original.as_slice()
  }

  pub fn matches(&self, str: &str) -> bool {
    self.re.is_match(str)
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    path.as_str().map_or(false, |s| {
      self.matches(s)
    })
  }

  pub fn escape(s: &str) -> String {
    let mut escaped = String::new();

    for c in s.chars() {
      match c {
        '?' | '*' | '[' | ']' => {
          escaped.push('[');
          escaped.push(c);
          escaped.push(']');
        }
        c => {
          escaped.push(c);
        }
      }
    }

    return escaped;
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
                // `**` begins with non-separator
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
          return Err(
            Error  {
              pos: i,
              msg: "invalid range pattern".to_string()});
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
        SingleChar(c) => {
          if c == '\\' {
            pattern.push_str(r"\\");
          } else {
            pattern.push(c)
          }
        },
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

  fn compile(tokens: Vec<Token>) -> Result<Regex, Error> {
    let mut re = String::new();

    for token in tokens.iter() {
      match *token {
        Char(c) => re.push_str(Pattern::escape_regex_char(c).as_slice()),
        AnyChar => re.push('.'),
        AnySequence =>
          re.push_str(
            format!(r"[^{sep}]*",
                    sep = Pattern::escape_regex_char(path::SEP).as_slice()).as_slice()),
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

    Regex::new(re.as_slice())
      .map_err(|e| Error { pos: e.pos, msg: e.msg })
  }
}

impl fmt::Show for Pattern {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.re)
  }
}

impl PartialEq for Pattern {
  fn eq(&self, other: &Pattern) -> bool {
    self.to_string() == other.to_string()
  }
}

#[cfg(test)]
mod test {
  use super::Pattern;

  #[test]
  fn match_dir() {
    let pat = Pattern::new("some/file.txt/").unwrap();
    assert!(pat.matches("some/file.txt/"));
    assert!(!pat.matches("some/file.txt"));
  }

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

    let pat = Pattern::new(r"cache/[[?*\]/files").unwrap();
    assert!(pat.re.is_match("cache/[/files"));
    assert!(pat.re.is_match("cache/?/files"));
    assert!(pat.re.is_match("cache/*/files"));
    assert!(pat.re.is_match(r"cache/\/files"));
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

  #[test]
  fn escape() {
    assert_eq!(
      Pattern::escape("one/?/two/*/three/[/four/]/end"),
      "one/[?]/two/[*]/three/[[]/four/[]]/end".to_string()
    );

    assert_eq!(
      Pattern::escape("one/?*[]"),
      "one/[?][*][[][]]".to_string()
    );
  }

  #[test]
  fn wildcards_two() {
    assert!(Pattern::new("a*b").unwrap().matches("a_b"));
    assert!(Pattern::new("a*b*c").unwrap().matches("abc"));
    assert!(!Pattern::new("a*b*c").unwrap().matches("abcd"));
    assert!(Pattern::new("a*b*c").unwrap().matches("a_b_c"));
    assert!(Pattern::new("a*b*c").unwrap().matches("a___b___c"));
    assert!(Pattern::new("abc*abc*abc").unwrap().matches("abcabcabcabcabcabcabc"));
    assert!(!Pattern::new("abc*abc*abc").unwrap().matches("abcabcabcabcabcabcabca"));
    assert!(Pattern::new("a*a*a*a*a*a*a*a*a").unwrap().matches("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    assert!(Pattern::new("a*b[xyz]c*d").unwrap().matches("abxcdbxcddd"));
  }

  #[test]
  fn recursive_wildcards() {
    let pat = Pattern::new("some/**/needle.txt").unwrap();
    assert!(pat.matches("some/needle.txt"));
    assert!(pat.matches("some/one/needle.txt"));
    assert!(pat.matches("some/one/two/needle.txt"));
    assert!(pat.matches("some/other/needle.txt"));
    assert!(!pat.matches("some/other/notthis.txt"));

    // a single ** should be valid, for globs
    Pattern::new("**").unwrap();

    // collapse consecutive wildcards
    let pat = Pattern::new("some/**/**/needle.txt").unwrap();
    assert!(pat.matches("some/needle.txt"));
    assert!(pat.matches("some/one/needle.txt"));
    assert!(pat.matches("some/one/two/needle.txt"));
    assert!(pat.matches("some/other/needle.txt"));
    assert!(!pat.matches("some/other/notthis.txt"));

    // ** can begin the pattern
    let pat = Pattern::new("**/test").unwrap();
    assert!(pat.matches("one/two/test"));
    assert!(pat.matches("one/test"));
    assert!(pat.matches("test"));

    // /** can begin the pattern
    let pat = Pattern::new("/**/test").unwrap();
    assert!(pat.matches("/one/two/test"));
    assert!(pat.matches("/one/test"));
    assert!(pat.matches("/test"));
    assert!(!pat.matches("/one/notthis"));
    assert!(!pat.matches("/notthis"));
  }

  #[test]
  fn range_pattern() {

    let pat = Pattern::new("a[0-9]b").unwrap();
    for i in range(0u, 10) {
      assert!(pat.matches(format!("a{}b", i).as_slice()));
    }
    assert!(!pat.matches("a_b"));

    let pat = Pattern::new("a[!0-9]b").unwrap();
    for i in range(0u, 10) {
      assert!(!pat.matches(format!("a{}b", i).as_slice()));
    }
    assert!(pat.matches("a_b"));

    let pats = ["[a-z123]", "[1a-z23]", "[123a-z]"];
    for &p in pats.iter() {
      let pat = Pattern::new(p).unwrap();
      for c in "abcdefghijklmnopqrstuvwxyz".chars() {
        assert!(pat.matches(c.to_string().as_slice()));
      }
      assert!(pat.matches("1"));
      assert!(pat.matches("2"));
      assert!(pat.matches("3"));
    }

    let pats = ["[abc-]", "[-abc]", "[a-c-]"];
    for &p in pats.iter() {
      let pat = Pattern::new(p).unwrap();
      assert!(pat.matches("a"));
      assert!(pat.matches("b"));
      assert!(pat.matches("c"));
      assert!(pat.matches("-"));
      assert!(!pat.matches("d"));
    }

    let pat = Pattern::new("[!1-2]").unwrap();
    assert!(!pat.matches("1"));
    assert!(!pat.matches("2"));

    assert!(Pattern::new("[-]").unwrap().matches("-"));
    assert!(!Pattern::new("[!-]").unwrap().matches("-"));
  }

  #[test]
  fn unclosed_bracket() {
    // TODO: assert error position
    // unclosed `[` should be treated literally
    assert!(Pattern::new("abc[def").is_err());
    assert!(Pattern::new("abc[!def").is_err());
    assert!(Pattern::new("abc[").is_err());
    assert!(Pattern::new("abc[!").is_err());
    assert!(Pattern::new("abc[d").is_err());
    assert!(Pattern::new("abc[!d").is_err());
    assert!(Pattern::new("abc[]").is_err());
    assert!(Pattern::new("abc[!]").is_err());
  }

  #[test]
  fn pattern_matches() {
    let txt_pat = Pattern::new("*hello.txt").unwrap();
    assert!(txt_pat.matches("hello.txt"));
    assert!(txt_pat.matches("gareth_says_hello.txt"));
    assert!(txt_pat.matches("some/path/to/hello.txt"));
    assert!(txt_pat.matches("some\\path\\to\\hello.txt"));
    assert!(txt_pat.matches("/an/absolute/path/to/hello.txt"));
    assert!(!txt_pat.matches("hello.txt-and-then-some"));
    assert!(!txt_pat.matches("goodbye.txt"));

    let dir_pat = Pattern::new("*some/path/to/hello.txt").unwrap();
    assert!(dir_pat.matches("some/path/to/hello.txt"));
    assert!(dir_pat.matches("a/bigger/some/path/to/hello.txt"));
    assert!(!dir_pat.matches("some/path/to/hello.txt-and-then-some"));
    assert!(!dir_pat.matches("some/other/path/to/hello.txt"));
  }

  #[test]
  fn pattern_escape() {
    let s = "_[_]_?_*_!_";
    assert_eq!(Pattern::escape(s), "_[[]_[]]_[?]_[*]_!_".to_string());
    assert!(Pattern::new(Pattern::escape(s).as_slice()).unwrap().matches(s));
  }

  #[test]
  fn matches_path() {
    // on windows, (Path::new("a/b").as_str().unwrap() == "a\\b"), so this
    // tests that / and \ are considered equivalent on windows
    assert!(Pattern::new("a/b").unwrap().matches_path(&Path::new("a/b")));
  }
}
