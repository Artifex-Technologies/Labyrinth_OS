// labyrinth-sarina-ask: a spoken question, from the recording to Sarina's answer (see src/sarina_voice.rs)
fn main() {
    std::process::exit(labyrinth_tools::sarina_voice::ask(&labyrinth_tools::py::args()));
}
