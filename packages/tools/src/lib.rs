//! Labyrinth OS's own programs in Rust, replacing the Python scripts of the same names (profile/airootfs/usr/local/bin) with the
//! same interface: arguments, output, files, environment variables and exit codes. Each module says what it is and where it is
//! not 1:1. Where a script would have stopped with a traceback (exit code 1), the Rust one stops too, with exit code 1 and one
//! line on standard error.
//!
//!   scan        labyrinth-scan-downloads   the malware scan of new files (always on: labyrinth-scan-watch.service)
//!   quarantine  labyrinth-quarantine       its quarantine and the way back

pub mod py;
pub mod pycompat;
pub mod quarantine;
pub mod scan;
