//! Reusable building blocks that know nothing about this application.
//!
//! Anything in here could be lifted into another service unchanged: it depends
//! on the standard library and third party crates, never on `module`, `shared`
//! or `config`. That one way dependency is the whole point of the folder, and
//! it is what keeps the business code out of the plumbing.

pub mod env;
