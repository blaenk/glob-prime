extern crate glob_prime;
extern crate glob;
extern crate test;

use test::Bencher;

use std::os;
use std::io;
use std::io::TempDir;

fn mk_file(path: &str, directory: bool) {
  if directory {
    io::fs::mkdir(&Path::new(path), io::USER_RWX).unwrap();
  } else {
    io::File::create(&Path::new(path)).unwrap();
  }
}

#[bench]
fn old_pattern(b: &mut Bencher) {
  use glob::Pattern;
  let pat = Pattern::new("one/two/three");
  b.iter(|| pat.matches("one/two/three"));
}

#[bench]
fn new_pattern(b: &mut Bencher) {
  use glob_prime::pattern::Pattern;
  let pat = Pattern::new("one/two/three").unwrap();
  b.iter(|| pat.matches("one/two/three"));
}

#[bench]
fn old_glob(b: &mut Bencher) {
  use glob::glob;

  fn glob_vec(pattern: &str) -> Vec<Path> {
    glob(pattern).collect()
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

  b.iter(|| glob_vec("**"));
}

#[bench]
fn new_impl(b: &mut Bencher) {
  use glob_prime::glob::glob;

  fn glob_vec(pattern: &str) -> Vec<Path> {
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

  b.iter(|| glob_vec("**/*"));
}

