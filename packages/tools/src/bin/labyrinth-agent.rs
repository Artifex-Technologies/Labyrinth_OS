// labyrinth-agent: the engineering agent's root actions, each checked (see src/agent.rs)
fn main() {
    std::process::exit(labyrinth_tools::agent::main(&labyrinth_tools::py::args_os()));
}
