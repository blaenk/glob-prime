use std::iter::Peekable;

use std::io::fs::PathExtensions;
use std::io::fs::readdir;

use std::path::is_sep;

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
    // TODO: should this be split on r"[^\]{SEP}"

    let mut patterns: Vec<&str> = Vec::new();
    let mut was_recursive = false;

    // collapse consecutive recursive patterns
    for pattern in pattern.split(is_sep) {
      if pattern == "**" {
        if was_recursive {
          continue;
        } else {
          was_recursive = true;
        }
      } else {
        was_recursive = false;
      }

      patterns.push(pattern);
    }

    return Selector::from_components(patterns.as_slice());
  }

  fn from_components(patterns: &[&str]) -> Result<Selector, Error> {
    if !patterns.is_empty() {
      let pattern = patterns[0];
      let rest = patterns.slice_from(1);

      if pattern == "**" {
        return Ok(Recursive {
          successor: Box::new(try!(Selector::from_components(rest))),
          directories: None,
        });
      }

      else if WILDCARD.is_match(pattern) {
        let compiled = try!(Pattern::new(pattern));
        return Ok(Wildcard {
          pattern: compiled,
          successor: Box::new(try!(Selector::from_components(rest))),
          entries: None,
        });
      }

      else {
        return Ok(Precise {
          pattern: pattern.to_string(),
          successor: Box::new(try!(Selector::from_components(rest))),
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

  fn select_from(&mut self, path: &Path, is_dir: bool) -> Option<Path> {
    match *self {
      Precise {
        ref pattern,
        successor: ref mut successor
      } => {
        let joined = path.join(pattern);

        if path.is_dir() && joined.exists() {
          return successor.select_from(&joined, is_dir);
        } else {
          return None;
        }
      },

      Wildcard {
        ref pattern,
        successor: ref mut successor,
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
            if is_dir && !entry.is_dir() {
              return None;
            }

            *entries = Some(ents);
            return Some(entry);
          }

          match successor.select_from(&entry, is_dir) {
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
        successor: ref mut successor,
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

          match successor.select_from(dirs.peek().unwrap(), is_dir) {
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

          if !is_dir || (is_dir && path.is_dir()) {
            return Some(path.clone());
          } else {
            return None;
          }
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
        match readdir(&path) {
          Ok(dirs) => {
            self.stack.extend(dirs.into_iter().filter(|p| p.is_dir()));
          }
          Err(..) => {}
        }

        Some(path)
      }
      None => None
    }
  }
}

fn walk_dir(path: &Path) -> ::std::io::IoResult<Directories> {
  Ok(Directories { stack: vec![path.clone()] })
}

pub struct Paths {
  scope: Path,
  selector: Selector,
  is_dir: bool,
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
  let root_len = root.as_ref().map_or(0us, |p| p.as_vec().len());

  if root.is_some() && check_windows_verbatim(root.as_ref().unwrap()) {
    panic!("FIXME: verbatim");
  }

  let scope =
    root
      .map(handle_volume_relative)
      .unwrap_or_else(|| Path::new("."));

  let trimmed = pattern.slice_from(min(root_len, pattern.len()));
  let selector = try!(Selector::from_pattern(trimmed));
  let is_dir = pattern.chars().next_back().map(is_sep) == Some(true);

  Ok(Paths {
    scope: scope,
    selector: selector,
    is_dir: is_dir,
  })
}

impl Iterator for Paths {
  type Item = Path;

  fn next(&mut self) -> Option<Path> {
    return self.selector.select_from(&self.scope, self.is_dir);
  }
}

#[cfg(test)]
mod test {
  use super::glob;

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
  fn absolute_pattern() {
    // assume that the filesystem is not empty!
    assert!(glob("/*").unwrap().next().is_some());
    assert!(glob("//").unwrap().next().is_some());

    // check windows absolute paths with host/device components
    let root_with_device = ::std::os::getcwd().unwrap().root_path().unwrap().join("*");
    // FIXME (#9639): This needs to handle non-utf8 paths
    assert!(glob(root_with_device.as_str().unwrap()).unwrap().next().is_some());
  }

  #[test]
  fn lots_of_files() {
    // TODO: this comes up with a perm denied file
    // this is a good test because it touches lots of differently named files
    // glob("/*/*/*/*").unwrap().skip(10000).next();
  }
}
