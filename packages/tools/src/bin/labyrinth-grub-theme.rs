// labyrinth-grub-theme: the boot menu theme (see src/grub_theme.rs)
fn main() {
    std::process::exit(labyrinth_tools::grub_theme::main(&labyrinth_tools::py::args()));
}
