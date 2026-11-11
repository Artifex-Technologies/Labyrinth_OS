// labyrinth-ecosystem-setup: Ollama, Sarina, Heimdall and Limbo wired up together (see src/ecosystem.rs)
fn main() {
    std::process::exit(labyrinth_tools::ecosystem::main(&labyrinth_tools::py::args()));
}
