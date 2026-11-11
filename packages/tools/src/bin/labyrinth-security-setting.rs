// labyrinth-security-setting: the security settings Settings asks for (see src/security_setting.rs)
fn main() {
    std::process::exit(labyrinth_tools::security_setting::main(&labyrinth_tools::py::args()));
}
