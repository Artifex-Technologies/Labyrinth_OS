// labyrinth-rea-setup: REA and Ghidra installed for labyrinth-rea (see src/rea.rs)
fn main() {
    std::process::exit(labyrinth_tools::rea::setup(&labyrinth_tools::py::args()));
}
