#![feature(phase)]

extern crate regex;

#[phase(plugin, link)]
extern crate regex_macros;

pub mod pattern;
