// labyrinth-quarantine: the quarantine of the malware scan (see src/quarantine.rs)
fn main() {
    std::process::exit(labyrinth_tools::quarantine::main(&labyrinth_tools::py::args()));
}
