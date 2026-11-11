// labyrinth-launch: the app launcher (see src/launch.rs)
fn main() {
    std::process::exit(labyrinth_tools::launch::main(&labyrinth_tools::py::args()));
}
