//! Kode-rs: AI-powered terminal assistant (Rust port)
//!
//! This library provides the core functionality for an AI-powered terminal assistant
//! that understands codebases, edits files, runs commands, and automates development workflows.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::too_many_lines)]

pub mod agents;
pub mod cli;
pub mod config;
pub mod error;
pub mod messages;
pub mod services;
pub mod tools;

// Re-exports for convenience
pub use error::{KodeError, Result};
