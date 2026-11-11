// labyrinth-install: the installer from the live desktop (see src/small.rs)
fn main() {
    std::process::exit(labyrinth_tools::small::install(&labyrinth_tools::py::args()));
}
