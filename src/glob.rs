use std::iter::Peekable;

use std::io::fs::PathExtensions;
use std::io::fs::readdir;

use std::cmp::min;

use pattern::{Pattern, Error};
use self::Selector::{Terminating, Precise, Wildcard, Recursive};

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
  Terminating {
    terminated: bool,
  },
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

    return Selector::from_components(patterns.as_slice());
  }

  fn from_components(patterns: &[&str]) -> Result<Selector, Error> {
    if !patterns.is_empty() {
      let pattern = patterns[0];
      let rest = patterns.slice_from(1);

      if pattern == "**" {
        return Ok(Recursive {
          successor: box try!(Selector::from_components(rest)),
          directories: None,
        });
      }

      else if WILDCARD.is_match(pattern) {
        let compiled = try!(Pattern::new(pattern));
        return Ok(Wildcard {
          pattern: compiled,
          successor: box try!(Selector::from_components(rest)),
          entries: None,
        });
      }

      else {
        return Ok(Precise {
          pattern: pattern.to_string(),
          successor: box try!(Selector::from_components(rest)),
        });
      }
    } else {
        return Ok(Terminating {
          terminated: false,
        });
    }
  }

  fn is_terminating(&self) -> bool {
    if let Terminating {..} = *self {
      true
    } else {
      false
    }
  }

  fn select_from(&mut self, path: &Path) -> Option<Path> {
    match *self {
      Precise {
        ref pattern,
        successor: box ref mut successor
      } => {
        let joined = path.join(pattern);

        if path.is_dir() && joined.exists() {
          return successor.select_from(&joined);
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
          entries.take().unwrap_or_else(|| readdir(path).unwrap());

        'outer: while let Some(entry) = ents.pop() {
          if !pattern.matches_path(&entry) {
            continue;
          }

          // this is necessary, otherwise the successor.select_from
          // would keep yielding Some(x) if the successor is Terminating
          if successor.is_terminating() {
            *entries = Some(ents);
            return Some(entry);
          }

          match successor.select_from(&entry) {
            None => continue 'outer,
            matched => {
              ents.push(entry);
              *entries = Some(ents);
              return matched;
            },
          }
        }

        return None;
      },

      // TODO: currently doesn't consider cur-dir
      Recursive {
        successor: box ref mut successor,
        ref mut directories,
      } => {
        if !path.is_dir() {
          return None;
        }

        let mut dirs =
          directories.take()
            .unwrap_or_else(|| walk_dir(path).unwrap().peekable());

        loop {
          if !dirs.peek().is_some() {
            return None;
          }

          // TODO:
          // this is returning only the directories,
          // like python, ruby, and zsh seems to do
          if successor.is_terminating() {
            let path = dirs.next();
            *directories = Some(dirs);
            return path;
          }

          match successor.select_from(dirs.peek().unwrap()) {
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

      // this is only used in the case of a Precise selector
      // followed by a Terminating selector. In this case,
      // the Precise selector would delegate to the Terminating
      // selector, which would continuously return Some(path)
      //
      // The `terminated` flag is used to prevent this, working
      // like a kind of semaphore which ensures that it returns
      // a given path once.
      Terminating {
        ref mut terminated,
      } => {
        if *terminated {
          *terminated = false;
          return None;
        } else {
          *terminated = true;
          return Some(path.clone());
        }
      },
    }
  }
}

struct Directories {
  stack: Vec<Path>,
}

impl Iterator for Directories {
  type Item = Path;

  fn next(&mut self) -> Option<Path> {
    match self.stack.pop() {
      Some(path) => {
        if path.is_dir() {
          match readdir(&path) {
            Ok(dirs) => {
              self.stack.extend(dirs.into_iter().filter(|p| p.is_dir()));
            }
            Err(..) => {}
          }
        }
        Some(path)
      }
      None => None
    }
  }
}

fn walk_dir(path: &Path) -> ::std::io::IoResult<Directories> {
  let mut dirs = try!(readdir(path));
  dirs.retain(|p| p.is_dir());

  Ok(Directories { stack: dirs })
}

pub struct Paths {
  scope: Path,
  selector: Selector,
}

pub fn glob(pattern: &str) -> Result<Paths, Error> {
  #[cfg(windows)]
  fn check_windows_verbatim(p: &Path) -> bool { path::windows::is_verbatim(p) }
  #[cfg(not(windows))]
  fn check_windows_verbatim(_: &Path) -> bool { false }

  #[cfg(windows)]
  fn handle_volume_relative(p: Path) -> Path {
    use std::os::getcwd;

    if path::windows::is_vol_relative(&p) {
      getcwd().unwrap().push(p);
    } else {
      p
    }
  }
  #[cfg(not(windows))]
  fn handle_volume_relative(p: Path) -> Path { p }

  let root = Path::new(pattern).root_path();
  let root_len = root.as_ref().map_or(0u, |p| p.as_vec().len());

  if root.is_some() && check_windows_verbatim(root.as_ref().unwrap()) {
    panic!("FIXME: verbatim");
  }

  let scope =
    root
      .map(handle_volume_relative)
      .unwrap_or_else(|| Path::new("."));

  let trimmed = pattern.slice_from(min(root_len, pattern.len()));
  let selector = try!(Selector::from_pattern(trimmed));

  Ok(Paths {
    scope: scope,
    selector: selector,
  })
}

impl Iterator for Paths {
  type Item = Path;

  fn next(&mut self) -> Option<Path> {
    return self.selector.select_from(&self.scope);
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
    // recursive and wildcard
    for (i, f) in glob("tests/fixtures/**/*.txt").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // wildcards
    for (i, f) in glob("s*rc/*.rs").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // the next three should be equivalent

    // end in recursive
    for (i, f) in glob("target/**").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // `..` end in recursive
    for (i, f) in glob("target/../target/**").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // a mess
    for (i, f) in glob("./target/./../target/**").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // absolute path
    for (i, f) in glob("/l*").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }

    // single file
    for (i, f) in glob("readme.md").unwrap().enumerate() {
      println!("-> {}. {}", i, f.display());
    }
  }
}
