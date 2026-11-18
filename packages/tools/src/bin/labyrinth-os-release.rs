// labyrinth-os-release: the system version in /etc/os-release, from the labyrinth-tools scriptlet (see src/os_release.rs)
fn main() {
    std::process::exit(labyrinth_tools::os_release::main(&labyrinth_tools::py::args()));
}
