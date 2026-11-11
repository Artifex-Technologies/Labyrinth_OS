//! Labyrinth OS's own programs in Rust, replacing the Python scripts of the same names (profile/airootfs/usr/local/bin) with the
//! same interface: arguments, output, files, environment variables and exit codes. Each module says what it is and where it is
//! not 1:1. Where a script would have stopped with a traceback (exit code 1), the Rust one stops too, with exit code 1 and one
//! line on standard error.
//!
//!   scan        labyrinth-scan-downloads    the malware scan of new files (always on: labyrinth-scan-watch.service)
//!   quarantine  labyrinth-quarantine        its quarantine and the way back
//!   appnet      labyrinth-appnet            per-app network rules (nftables)
//!   appnet_ask  labyrinth-appnet-ask        the ask mode's questions (always on: a root service)
//!   ask_agent   labyrinth-appnet-ask-agent  the ask mode's notifications (always on: a user service)
//!   launch      labyrinth-launch            the app launcher (dock, start menu, Settings)
//!   apps        labyrinth-apps              the optional software (Agora, the first login)
//!   selfcheck   labyrinth-selfcheck         is the desktop in a sane state (after an upgrade)
//!   ipconfig    ipconfig                    Windows' ipconfig from ip and NetworkManager
//!   playall     steam_playall               Steam Play for all titles in Steam's config.vdf (labyrinth-gaming setup)
//!   first_components  labyrinth-first-components  the optional parts chosen in the installer, at the first login

pub mod appnet;
pub mod appnet_ask;
pub mod apps;
pub mod ask_agent;
pub mod first_components;
pub mod ipconfig;
pub mod launch;
pub mod playall;
pub mod py;
pub mod pycompat;
pub mod pyre;
pub mod quarantine;
pub mod scan;
pub mod selfcheck;

#[cfg(test)]
pub mod testutil;
