#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::if_same_then_else)]

#[macro_use]
mod any;

pub mod config;
pub mod git;
pub mod graph;
pub mod rewrite;

pub mod legacy;
