// steam_playall: Steam Play for all titles (see src/playall.rs)
fn main() {
    std::process::exit(labyrinth_tools::playall::main(&labyrinth_tools::py::args()));
}
