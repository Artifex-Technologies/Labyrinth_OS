// labyrinth-sarina: Sarina with a model server behind her (see src/sarina.rs)
fn main() {
    std::process::exit(labyrinth_tools::sarina::main(&labyrinth_tools::py::args()));
}
