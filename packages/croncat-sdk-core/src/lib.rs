#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

pub mod error;
pub mod internal_messages;
#[cfg(test)]
mod tests;
pub mod types;
