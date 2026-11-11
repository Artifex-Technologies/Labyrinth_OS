// labyrinth-first-components: the optional parts chosen in the installer, at the first login (see src/first_components.rs)
fn main() {
    std::process::exit(labyrinth_tools::first_components::main(&labyrinth_tools::py::args()));
}
