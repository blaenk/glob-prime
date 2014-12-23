This is an attempt at rewriting the [glob](http://doc.rust-lang.org/glob/glob/index.html) crate for Rust. The focus is on:

* simpler and more flexible globbing algorithm
* error reporting on `Pattern` construction
* converting patterns to regular expressions under the hood in order to leverage [existing infrastructure](http://doc.rust-lang.org/regex/regex/index.html) and reduce the surface area of the globbing implementation

