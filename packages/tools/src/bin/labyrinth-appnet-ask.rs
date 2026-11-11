// labyrinth-appnet-ask: the questions of the ask mode (see src/appnet_ask.rs)
fn main() {
    std::process::exit(labyrinth_tools::appnet_ask::main(&labyrinth_tools::py::args()));
}
