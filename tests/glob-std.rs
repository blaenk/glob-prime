// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-windows TempDir may cause IoError on windows: #10462

extern crate glob_prime;

use glob_prime::glob::glob;
use std::os;
use std::io;
use std::io::TempDir;

use std::collections::HashSet;

macro_rules! assert_eq {
  ($e1:expr, $e2:expr) => (
    if $e1 != $e2 {
      panic!("{} != {}", stringify!($e1), stringify!($e2))
    }
  )
}

macro_rules! set {
  ($($e:expr),*) => ({
    let mut _temp = ::std::collections::HashSet::new();
    $(_temp.insert($e);)*
      _temp
  })
}

#[test]
fn main() {
  fn mk_file(path: &str, directory: bool) {
    if directory {
      io::fs::mkdir(&Path::new(path), io::USER_RWX).unwrap();
    } else {
      io::File::create(&Path::new(path)).unwrap();
    }
  }

  fn glob_set(pattern: &str) -> HashSet<Path> {
    glob(pattern).unwrap().collect()
  }

  let root = TempDir::new("glob-tests");
  let root = root.ok().expect("Should have created a temp directory");
  assert!(os::change_dir(root.path()).is_ok());

  mk_file("aaa", true);
  mk_file("aaa/apple", true);
  mk_file("aaa/orange", true);
  mk_file("aaa/tomato", true);
  mk_file("aaa/tomato/tomato.txt", false);
  mk_file("aaa/tomato/tomoto.txt", false);
  mk_file("bbb", true);
  mk_file("bbb/specials", true);
  mk_file("bbb/specials/!", false);

  // windows does not allow `*` or `?` characters to exist in filenames
  if os::consts::FAMILY != "windows" {
    mk_file("bbb/specials/*", false);
    mk_file("bbb/specials/?", false);
  }

  mk_file("bbb/specials/[", false);
  mk_file("bbb/specials/]", false);
  mk_file("ccc", true);
  mk_file("xyz", true);
  mk_file("xyz/x", false);
  mk_file("xyz/y", false);
  mk_file("xyz/z", false);

  mk_file("r", true);
  mk_file("r/current_dir.md", false);
  mk_file("r/one", true);
  mk_file("r/one/a.md", false);
  mk_file("r/one/another", true);
  mk_file("r/one/another/a.md", false);
  mk_file("r/another", true);
  mk_file("r/another/a.md", false);
  mk_file("r/two", true);
  mk_file("r/two/b.md", false);
  mk_file("r/three", true);
  mk_file("r/three/c.md", false);

  // all recursive entities
  assert_eq!(glob_set("r/**"), set!(
    Path::new("r"),
    Path::new("r/one"),
    Path::new("r/one/another"),
    Path::new("r/another"),
    Path::new("r/two"),
    Path::new("r/three")));

  // collapse consecutive recursive patterns
  assert_eq!(glob_set("r/**/**"), set!(
    Path::new("r"),
    Path::new("r/one"),
    Path::new("r/one/another"),
    Path::new("r/another"),
    Path::new("r/two"),
    Path::new("r/three")));

  // followed by a wildcard
  assert_eq!(glob_set("r/**/*.md"), set!(
    Path::new("r/another/a.md"),
    Path::new("r/current_dir.md"),
    Path::new("r/one/a.md"),
    Path::new("r/one/another/a.md"),
    Path::new("r/three/c.md"),
    Path::new("r/two/b.md")));

  // followed by a precise pattern
  assert_eq!(glob_set("r/one/**/a.md"), set!(
    Path::new("r/one/a.md"),
    Path::new("r/one/another/a.md")));

  // followed by another recursive pattern
  // collapses consecutive recursives into one
  assert_eq!(glob_set("r/one/**/**/a.md"), set!(
    Path::new("r/one/a.md"),
    Path::new("r/one/another/a.md")));

  // followed by two precise patterns
  assert_eq!(glob_set("r/**/another/a.md"), set!(
    Path::new("r/another/a.md"),
    Path::new("r/one/another/a.md")));

  // TODO: fix
  // assert_eq!(glob_set(""), set!());
  // TODO: this seems weird
  assert_eq!(glob_set("."), set!(Path::new(".")));
  assert_eq!(glob_set(".."), set!(Path::new("..")));

  assert_eq!(glob_set("aaa"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aaa/"), set!(Path::new("aaa")));
  assert_eq!(glob_set("a"), set!());
  assert_eq!(glob_set("aa"), set!());
  assert_eq!(glob_set("aaaa"), set!());

  assert_eq!(glob_set("aaa/apple"), set!(Path::new("aaa/apple")));
  assert_eq!(glob_set("aaa/apple/nope"), set!());

  // windows should support both / and \ as directory separators
  if os::consts::FAMILY == "windows" {
    assert_eq!(glob_set("aaa\\apple"), set!(Path::new("aaa/apple")));
  }

  assert_eq!(glob_set("???/"), set!(
    Path::new("aaa"),
    Path::new("bbb"),
    Path::new("ccc"),
    Path::new("xyz")));

  assert_eq!(glob_set("aaa/tomato/tom?to.txt"), set!(
    Path::new("aaa/tomato/tomato.txt"),
    Path::new("aaa/tomato/tomoto.txt")));

  assert_eq!(glob_set("xyz/?"), set!(
    Path::new("xyz/x"),
    Path::new("xyz/y"),
    Path::new("xyz/z")));

  assert_eq!(glob_set("a*"), set!(Path::new("aaa")));
  assert_eq!(glob_set("*a*"), set!(Path::new("aaa")));
  assert_eq!(glob_set("a*a"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aaa*"), set!(Path::new("aaa")));
  assert_eq!(glob_set("*aaa"), set!(Path::new("aaa")));
  assert_eq!(glob_set("*aaa*"), set!(Path::new("aaa")));
  assert_eq!(glob_set("*a*a*a*"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aaa*/"), set!(Path::new("aaa")));

  assert_eq!(glob_set("aaa/*"), set!(
    Path::new("aaa/apple"),
    Path::new("aaa/orange"),
    Path::new("aaa/tomato")));

  assert_eq!(glob_set("aaa/*a*"), set!(
    Path::new("aaa/apple"),
    Path::new("aaa/orange"),
    Path::new("aaa/tomato")));

  assert_eq!(glob_set("*/*/*.txt"), set!(
    Path::new("aaa/tomato/tomato.txt"),
    Path::new("aaa/tomato/tomoto.txt")));

  assert_eq!(glob_set("*/*/t[aob]m?to[.]t[!y]t"), set!(
    Path::new("aaa/tomato/tomato.txt"),
    Path::new("aaa/tomato/tomoto.txt")));

  assert_eq!(glob_set("./aaa"), set!(Path::new("aaa")));
  assert_eq!(glob_set("./*"), glob_set("*"));
  // TODO: what
  // assert_eq!(glob_set("*/..").pop().unwrap(), Path::new("."));
  assert_eq!(glob_set("aaa/../bbb"), set!(Path::new("bbb")));
  assert_eq!(glob_set("nonexistent/../bbb"), set!());
  assert_eq!(glob_set("aaa/tomato/tomato.txt/.."), set!());

  assert_eq!(glob_set("aaa/tomato/tomato.txt/"), set!());

  assert_eq!(glob_set("aa[a]"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aa[abc]"), set!(Path::new("aaa")));
  assert_eq!(glob_set("a[bca]a"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aa[b]"), set!());
  assert_eq!(glob_set("aa[xyz]"), set!());
  assert_eq!(glob_set("aa[]]"), set!());

  assert_eq!(glob_set("aa[!b]"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aa[!bcd]"), set!(Path::new("aaa")));
  assert_eq!(glob_set("a[!bcd]a"), set!(Path::new("aaa")));
  assert_eq!(glob_set("aa[!a]"), set!());
  assert_eq!(glob_set("aa[!abc]"), set!());

  assert_eq!(glob_set("bbb/specials/[[]"), set!(Path::new("bbb/specials/[")));
  assert_eq!(glob_set("bbb/specials/!"), set!(Path::new("bbb/specials/!")));
  assert_eq!(glob_set("bbb/specials/[]]"), set!(Path::new("bbb/specials/]")));

  if os::consts::FAMILY != "windows" {
    assert_eq!(glob_set("bbb/specials/[*]"), set!(Path::new("bbb/specials/*")));
    assert_eq!(glob_set("bbb/specials/[?]"), set!(Path::new("bbb/specials/?")));
  }

  if os::consts::FAMILY == "windows" {
    assert_eq!(glob_set("bbb/specials/[![]"), set!(
        Path::new("bbb/specials/!"),
        Path::new("bbb/specials/]")));

    assert_eq!(glob_set("bbb/specials/[!]]"), set!(
        Path::new("bbb/specials/!"),
        Path::new("bbb/specials/[")));

    assert_eq!(glob_set("bbb/specials/[!!]"), set!(
        Path::new("bbb/specials/["),
        Path::new("bbb/specials/]")));
  } else {
    assert_eq!(glob_set("bbb/specials/[![]"), set!(
      Path::new("bbb/specials/!"),
      Path::new("bbb/specials/*"),
      Path::new("bbb/specials/?"),
      Path::new("bbb/specials/]")));

    assert_eq!(glob_set("bbb/specials/[!]]"), set!(
      Path::new("bbb/specials/!"),
      Path::new("bbb/specials/*"),
      Path::new("bbb/specials/?"),
      Path::new("bbb/specials/[")));

    assert_eq!(glob_set("bbb/specials/[!!]"), set!(
      Path::new("bbb/specials/*"),
      Path::new("bbb/specials/?"),
      Path::new("bbb/specials/["),
      Path::new("bbb/specials/]")));

    assert_eq!(glob_set("bbb/specials/[!*]"), set!(
      Path::new("bbb/specials/!"),
      Path::new("bbb/specials/?"),
      Path::new("bbb/specials/["),
      Path::new("bbb/specials/]")));

    assert_eq!(glob_set("bbb/specials/[!?]"), set!(
      Path::new("bbb/specials/!"),
      Path::new("bbb/specials/*"),
      Path::new("bbb/specials/["),
      Path::new("bbb/specials/]")));
  }
}
