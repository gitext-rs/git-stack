//! > DESCRIPTION

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[macro_use]
pub mod any;

pub mod config;
pub mod git;
pub mod graph;
pub mod rewrite;

pub mod legacy;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
