// labyrinth-rea: REA in a sandbox (see src/rea.rs)
fn main() {
    std::process::exit(labyrinth_tools::rea::rea(&labyrinth_tools::py::args()));
}
