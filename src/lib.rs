#![feature(plugin)]

extern crate regex;

#[plugin]
extern crate regex_macros;

pub mod pattern;
pub mod glob;
