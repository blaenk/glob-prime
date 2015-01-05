use pattern::{Pattern, Error};
use self::Selector::{Terminating, Precise, Wildcard, Recursive};

use std::io::fs::{Directories, walk_dir};
use std::iter::Peekable;

enum Selector {
  Precise {
    pattern: String,
    successor: Box<Selector>,
  },
  Wildcard {
    pattern: Pattern,
    successor: Box<Selector>,

    // book keeping
    entries: Option<Vec<Path>>,
  },
  Recursive {
    successor: Box<Selector>,

    // book keeping
    directories: Option<Peekable<Path, Directories>>,
  },
  Terminating,
}

static WILDCARD: ::regex::Regex = regex!(r"[[*?]");

impl Selector {
  // better name for this? perhaps surprising it returns a vec since name is
  // Selector::from_pattern("blah")
  fn from_pattern(pattern: &str) -> Result<Selector, Error> {
    // compile pattern to make sure there are no immediate errors
    let _compiled = try!(Pattern::new(pattern));
    let patterns =
      pattern.split_terminator(::std::path::SEP)
        .collect::<Vec<&str>>();

    fn build_selector(patterns: &[&str]) -> Result<Selector, Error> {
      if !patterns.is_empty() {
        let pattern = patterns[0];
        let rest = patterns.slice_from(1);

        if pattern == "**" {
          return Ok(Recursive {
            successor: box try!(build_selector(rest)),
            directories: None,
          });
        }

        else if WILDCARD.is_match(pattern) {
          let compiled = try!(Pattern::new(pattern));
          return Ok(Wildcard {
            pattern: compiled,
            successor: box try!(build_selector(rest)),
            entries: None,
          });
        }

        else {
          return Ok(Precise {
            pattern: pattern.to_string(),
            successor: box try!(build_selector(rest)),
          });
        }
      } else {
        return Ok(Terminating);
      }
    }

    return build_selector(patterns.as_slice());
  }

  fn select_from(&mut self, path: Path) -> Option<Path> {
    use std::io::fs::PathExtensions;
    use std::io::fs::readdir;

    match *self {
      Precise { pattern: ref pat, successor: box ref mut succ } => {
        let joined = path.join(pat);

        if path.is_dir() && joined.exists() {
          return succ.select_from(joined);
        } else {
          return None;
        }
      },

      Wildcard {
        ref pattern,
        successor: box ref mut successor,
        ref mut entries,
      } => {
        if !path.is_dir() {
          return None;
        }

        let mut ents =
          entries.take().unwrap_or_else(|| readdir(&path).unwrap());

        'outer: while let Some(entry) = ents.pop() {
          if !pattern.matches_path(&entry) {
            continue;
          }

          // this is necessary, otherwise the successor.select_from
          // would keep yielding Some(x) if the successor is Terminating
          if let &Terminating = successor {
            *entries = Some(ents);
            return Some(entry);
          }

          else {
            loop {
              let current = entry.clone();
              match successor.select_from(entry) {
                None => continue 'outer,
                matched => {
                  ents.push(current);
                  *entries = Some(ents);
                  return matched;
                },
              }
            }
          }
        }

        return None;
      },

      Recursive {
        successor: box ref mut successor,
        ref mut directories,
      } => {
        if !path.is_dir() {
          return None;
        }

        let mut dirs =
          directories.take()
            .unwrap_or_else(|| walk_dir(&path).unwrap().peekable());

        loop {
          if !dirs.peek().is_some() {
            return None;
          }

          let current = dirs.peek().unwrap().clone();

          match successor.select_from(current) {
            None => {
              dirs.next();
              continue;
            },
            matched => {
              *directories = Some(dirs);
              return matched;
            }
          }
        }
      },

      Terminating => Some(path),
    }
  }
}

pub struct Paths {
  selector: Selector,
}

pub fn glob(pattern: &str) -> Result<Paths, Error> {
  let selector = try!(Selector::from_pattern(pattern));

  Ok(Paths { selector: selector })
}

impl Iterator for Paths {
  type Item = Path;

  fn next(&mut self) -> Option<Path> {
    return self.selector.select_from(Path::new("."));
  }
}

#[cfg(test)]
mod test {
  use pattern::Pattern;
  use super::{Selector, glob};
  use super::Selector::*;

  // #[test]
  // fn selectors() {
  //   let selector = Selector::from_pattern("one/tw*/**/four").unwrap();
  //   assert_eq!(
  //     selector,
  //     Precise {
  //       pattern: "one".to_string(),
  //       successor: box

  //         Wildcard {
  //           pattern: Pattern::new("tw*").unwrap(),
  //           entries: vec![],
  //           index: 0,
  //           successor: box

  //             Recursive {
  //               successor: box

  //                 Precise {
  //                   pattern: "four".to_string(),
  //                   successor: box Terminating}}}});
  // }

  #[test]
  fn wildcards() {
    for (i, f) in glob("tests/fixtures/**/*.txt").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }
  }
}
