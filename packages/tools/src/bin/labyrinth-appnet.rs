// labyrinth-appnet: per-app network rules (see src/appnet.rs)
fn main() {
    std::process::exit(labyrinth_tools::appnet::main(&labyrinth_tools::py::args()));
}
