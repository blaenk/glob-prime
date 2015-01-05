#![feature(phase)]
#![feature(globs)]
#![feature(associated_types)]

extern crate regex;

#[phase(plugin, link)]
extern crate regex_macros;

pub mod pattern;
pub mod glob;
