//!     labyrinth-publish-repo            build and publish what changed
//!     labyrinth-publish-repo --force    build and publish even if nothing changed
//!     labyrinth-publish-repo --dry-run  build and sign, show what would be uploaded, upload nothing
//!     labyrinth-publish-repo build [--force] [NAME...]  the ISO's local repository, from this checkout (src/build.rs)

use labyrinth_publish_repo::args::{parse, Parsed};
use labyrinth_publish_repo::publish::{publish, Options};
use std::ffi::OsString;
use std::path::Path;

fn main() {
    let argv: Vec<OsString> = std::env::args_os().collect();
    let prog = argv
        .first()
        .and_then(|a| Path::new(a).file_name())
        .map_or_else(|| "labyrinth-publish-repo".to_string(), |n| n.to_string_lossy().into_owned());
    let code = match parse(&prog, argv.get(1..).unwrap_or_default()) {
        Parsed::Help(text) => {
            print!("{text}");
            0
        }
        Parsed::Refused(text) => {
            eprint!("{text}");
            2
        }
        Parsed::Run { force, dry_run } => match publish(&Options { force, dry_run }) {
            Ok(failed) if failed.is_empty() => 0,
            Ok(failed) => {
                let rest = if dry_run { "the others are in the dry run's repository" } else { "the others were published" };
                eprintln!("error: {} did not build (each is named above with why); {rest}", failed.join(", "));
                1
            }
            Err(msg) => {
                eprintln!("{msg}");
                1
            }
        },
        Parsed::Build { names, force } => match labyrinth_publish_repo::build::local(&names, force) {
            Ok(0) => 0,
            Ok(n) => {
                eprintln!("error: {n} recipe(s) did not build (each is named above with why); the local repository has the rest");
                1
            }
            Err(msg) => {
                eprintln!("{msg}");
                1
            }
        },
    };
    std::process::exit(code);
}
