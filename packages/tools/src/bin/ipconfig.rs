// ipconfig: Windows' ipconfig (see src/ipconfig.rs)
fn main() {
    std::process::exit(labyrinth_tools::ipconfig::main(&labyrinth_tools::py::args()));
}
