// labyrinth-apps: the optional software (see src/apps.rs)
fn main() {
    std::process::exit(labyrinth_tools::apps::main(&labyrinth_tools::py::args()));
}
