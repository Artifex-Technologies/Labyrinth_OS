// labyrinth-selfcheck: is the desktop in a sane state (see src/selfcheck.rs)
fn main() {
    std::process::exit(labyrinth_tools::selfcheck::main(&labyrinth_tools::py::args()));
}
