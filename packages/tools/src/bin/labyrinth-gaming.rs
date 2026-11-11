// labyrinth-gaming: are Steam and Proton installed and working (see src/gaming.rs)
fn main() {
    std::process::exit(labyrinth_tools::gaming::main(&labyrinth_tools::py::args()));
}
