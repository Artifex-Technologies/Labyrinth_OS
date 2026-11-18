// labyrinth-bluetooth-privacy: Privacy = device in BlueZ's main.conf, from the labyrinth-tools scriptlet and hook (see
// src/bt_privacy.rs)
fn main() {
    std::process::exit(labyrinth_tools::bt_privacy::main(&labyrinth_tools::py::args()));
}
