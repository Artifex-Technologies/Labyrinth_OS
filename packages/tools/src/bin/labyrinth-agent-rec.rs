// labyrinth-agent-rec: the recorder of the engineering agent's root sessions (see src/agent_rec.rs)
fn main() {
    std::process::exit(labyrinth_tools::agent_rec::main(&labyrinth_tools::py::args_os()));
}
