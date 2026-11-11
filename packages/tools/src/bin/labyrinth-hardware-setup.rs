// labyrinth-hardware-setup: only the drivers and device tools this computer needs (see src/hardware.rs)
fn main() {
    std::process::exit(labyrinth_tools::hardware::main(&labyrinth_tools::py::args()));
}
