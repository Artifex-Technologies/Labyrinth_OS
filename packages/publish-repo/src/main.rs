//!     labyrinth-publish-repo            build and publish what changed
//!     labyrinth-publish-repo --force    build and publish even if nothing changed
//!     labyrinth-publish-repo --dry-run  build and sign, show what would be uploaded, upload nothing

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
            Ok(()) => 0,
            Err(msg) => {
                eprintln!("{msg}");
                1
            }
        },
    };
    std::process::exit(code);
}
