// labyrinth-agent-setup: the engineering agent's own account and its sudo rules (see src/agent_setup.rs)
fn main() {
    std::process::exit(labyrinth_tools::agent_setup::main(&labyrinth_tools::py::args()));
}
