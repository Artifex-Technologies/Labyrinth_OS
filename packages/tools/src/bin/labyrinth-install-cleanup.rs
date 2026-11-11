// labyrinth-install-cleanup: the live medium's pieces out of the installed system, the kernels back (see src/install_cleanup.rs)
fn main() {
    std::process::exit(labyrinth_tools::install_cleanup::main());
}
