//! labyrinth-sarina [chat|gui|model]: starts Sarina (the local-model agent) with Ollama behind it: Ollama offered for install
//! when missing (CUDA build on NVIDIA, Vulkan otherwise), `ollama serve` started for the user with a 16k context when nothing
//! answers on 127.0.0.1:11434, a model that fits the memory offered when none is installed. Never downloads without asking. The
//! bash script of the same name with the same output, questions, calls and exit codes.
//!
//! curl (is the server up), ollama, sudo, pacman, lspci, xdg-open and sarina are still those programs; the memory size
//! (/proc/meminfo), the model list's first name (awk) and the waits are done here.
//!
//! Not 1:1: with HOME unset, chat passes an empty --cwd (bash stopped on the unbound variable).

use crate::sh::{self, Io};

const OLLAMA_URL: &str = "http://127.0.0.1:11434";

fn say(m: &str) {
    println!("{m}");
}

/// The model that fits: about a third of the memory is a safe ceiling for the weights and the context.
pub fn suggest(meminfo: &str) -> &'static str {
    let mut gb = String::new();
    for line in meminfo.split('\n').filter(|l| l.starts_with("MemTotal:")) {
        let kb: f64 = line.split([' ', '\t']).filter(|f| !f.is_empty()).nth(1).and_then(|f| f.parse().ok()).unwrap_or(0.0);
        gb.push_str(&format!("{}", (kb / 1048576.0).trunc() as i64));
    }
    let gb: i64 = gb.parse().unwrap_or(0);
    match gb {
        g if g >= 48 => "qwen3:30b",
        g if g >= 24 => "qwen3:14b",
        g if g >= 12 => "qwen3:8b",
        _ => "qwen3:4b",
    }
}

fn server_up() -> bool {
    sh::quiet(&["curl", "-fsS", "-m", "2", &format!("{OLLAMA_URL}/api/tags")])
}

fn yes(a: &str) -> bool {
    matches!(a, "y" | "Y" | "yes")
}

fn start_server() -> Result<(), i32> {
    if server_up() {
        return Ok(());
    }
    say("Starting the model server (Ollama)...");
    let dir = std::env::var("XDG_RUNTIME_DIR").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/tmp".into());
    let log = format!("{dir}/ollama-serve.log");
    if let Err(e) = sh::detached(&["ollama", "serve"], &[("OLLAMA_CONTEXT_LENGTH", "16384")], &log) {
        eprintln!("labyrinth-sarina: {log}: {e}");
    }
    for _ in 0..30 {
        if server_up() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    say(&format!("labyrinth-sarina: the model server did not start. See {log}"));
    Err(1)
}

/// The first word of `ollama list`'s second line (the first model), as awk 'NR>1 {print $1}' | head -n1 gives it.
pub fn first_model(list: &str) -> String {
    list.split('\n').nth(1).and_then(|l| l.split([' ', '\t']).find(|w| !w.is_empty())).unwrap_or("").to_string()
}

fn ensure_model() -> Result<(), i32> {
    if std::env::var("OPENAI_MODEL").map(|m| !m.is_empty()).unwrap_or(false) {
        return Ok(());
    }
    let have = first_model(&String::from_utf8_lossy(&sh::capture(&["ollama", "list"], Io::Null).1));
    if !have.is_empty() {
        std::env::set_var("OPENAI_MODEL", &have);
        return Ok(());
    }
    let m = suggest(&String::from_utf8_lossy(&std::fs::read("/proc/meminfo").unwrap_or_default()));
    say(&format!("No model is installed yet. For this computer I suggest: {m}"));
    if yes(&sh::ask(&format!("Download {m} now? It can be several gigabytes. [y/N] "))) {
        if sh::status(&["ollama", "pull", m]) != 0 {
            return Err(1);
        }
    } else {
        say("Run 'ollama pull <model>' when you are ready, then start Sarina again.");
        return Err(0);
    }
    std::env::set_var("OPENAI_MODEL", m);
    Ok(())
}

/// Is there an NVIDIA graphics card (its driver loaded, or lspci lists one)?
fn nvidia() -> bool {
    if sh::exists("/proc/driver/nvidia/version") {
        return true;
    }
    let (_, out) = sh::capture(&["lspci"], Io::Null);
    String::from_utf8_lossy(&out)
        .to_lowercase()
        .split('\n')
        .any(|l| ["vga", "3d"].iter().any(|p| l.find(p).map(|i| l[i + p.len()..].contains("nvidia")).unwrap_or(false)))
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-sarina");
    match run(args) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn run(args: &[String]) -> Result<(), i32> {
    std::env::set_var("OPENAI_BASE_URL", format!("{OLLAMA_URL}/v1"));
    if std::env::var("OPENAI_API_KEY").map(|k| k.is_empty()).unwrap_or(true) {
        std::env::set_var("OPENAI_API_KEY", "local-token");
    }
    if !sh::have("sarina") {
        say("labyrinth-sarina: sarina is not installed (install the sarina-git package)");
        return Err(1);
    }
    // the model server is not in the image (with CUDA it is about 6 GB): offered on first use
    if !sh::have("ollama") {
        let (pkg, size) =
            if nvidia() { ("ollama-cuda", "about 6 GB with NVIDIA's CUDA libraries") } else { ("ollama-vulkan", "a few hundred MB") };
        say(&format!("Sarina needs a model server (Ollama). For this computer: {pkg} ({size})."));
        if yes(&sh::ask(&format!("Install {pkg} now? It asks for your password. [y/N] "))) {
            if sh::status(&["sudo", "pacman", "-S", "--needed", pkg]) != 0 {
                return Err(1);
            }
        } else {
            say(&format!("Install it later with: sudo pacman -S {pkg}"));
            return Err(0);
        }
    }
    match args.get(1).map(|s| s.as_str()).filter(|s| !s.is_empty()).unwrap_or("chat") {
        "chat" => {
            start_server()?;
            ensure_model()?;
            let home = std::env::var("HOME").unwrap_or_default();
            Err(sh::exec(&["sarina", "chat", "--interactive", "--cwd", &home]))
        }
        "gui" => {
            start_server()?;
            ensure_model()?;
            sh::later(2, &["xdg-open", "http://127.0.0.1:8765"]);
            Err(sh::exec(&["sarina", "gui"]))
        }
        "model" => {
            start_server()?;
            std::env::remove_var("OPENAI_MODEL");
            ensure_model()
        }
        _ => {
            say("usage: labyrinth-sarina [chat|gui|model]");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn models() {
        assert_eq!(super::suggest("MemTotal: 65536000 kB\n"), "qwen3:30b");
        assert_eq!(super::suggest("MemTotal: 16000000 kB\n"), "qwen3:8b");
        assert_eq!(super::suggest(""), "qwen3:4b");
        assert_eq!(super::first_model("NAME ID\nqwen3:8b  x\nother y\n"), "qwen3:8b");
        assert_eq!(super::first_model("NAME ID\n\nother y\n"), "");
        assert_eq!(super::first_model(""), "");
    }
}
