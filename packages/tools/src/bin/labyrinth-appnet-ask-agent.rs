// labyrinth-appnet-ask-agent: the notifications of the ask mode (see src/ask_agent.rs)
fn main() {
    std::process::exit(labyrinth_tools::ask_agent::main(&labyrinth_tools::py::args()));
}
