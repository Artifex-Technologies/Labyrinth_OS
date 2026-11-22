//! labyrinth-publish-repo: Labyrinth's package repository, built, signed and put online (the release "packages" of
//! github.com/Artifex-Technologies/Labyrinth_OS, the [labyrinth] repository every installed system updates from). The Rust
//! port of scripts/publish-repo.py (2026-11-13), with the same options, steps, calls, files and messages; see README.md.

pub mod args;
pub mod cmd;
pub mod pkg;
pub mod publish;
pub mod py;
