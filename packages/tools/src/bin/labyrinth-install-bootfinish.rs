// labyrinth-install-bootfinish: the installer's last step, in the installed system (see src/bootfinish.rs)
fn main() {
    std::process::exit(labyrinth_tools::bootfinish::main());
}
