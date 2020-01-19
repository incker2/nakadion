#[cfg(feature = "metrix")]
extern crate metrix;

pub(crate) mod env_vars;
pub(crate) mod helpers;

mod internals;

pub mod auth;
pub mod event_stream;
//pub mod handler;
pub mod model;
pub mod nakadi_api;

pub mod nakadi_types;
