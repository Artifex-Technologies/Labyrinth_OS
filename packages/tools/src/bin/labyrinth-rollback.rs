// labyrinth-rollback: the whole system back to a snapshot, and back again (see src/rollback.rs)
fn main() {
    std::process::exit(labyrinth_tools::rollback::main(&labyrinth_tools::py::args()));
}
