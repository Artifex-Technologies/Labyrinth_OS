// labyrinth-sarina-speak: Sarina's answer said aloud (see src/sarina_voice.rs)
fn main() {
    std::process::exit(labyrinth_tools::sarina_voice::speak(&labyrinth_tools::py::args_os()));
}
