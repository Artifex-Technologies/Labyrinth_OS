// labyrinth-scan-downloads: the malware scan of new files (see src/scan.rs)
fn main() {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    std::process::exit(labyrinth_tools::scan::main(&argv));
}
