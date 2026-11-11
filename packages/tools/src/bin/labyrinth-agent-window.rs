// labyrinth-agent-window: the owner opens the agent's broad root commands for some hours (see src/agent_window.rs)
fn main() {
    std::process::exit(labyrinth_tools::agent_window::main(&labyrinth_tools::py::args()));
}
