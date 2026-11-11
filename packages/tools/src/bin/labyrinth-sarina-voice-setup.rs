// labyrinth-sarina-voice-setup: Sarina's voice installed (see src/sarina_small.rs)
fn main() {
    std::process::exit(labyrinth_tools::sarina_small::voice_setup(&labyrinth_tools::py::args()));
}
