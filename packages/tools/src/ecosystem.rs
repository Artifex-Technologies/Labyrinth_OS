//! labyrinth-ecosystem-setup: wires up Ollama, Sarina, Heimdall and Limbo so they work together (run as root; idempotent: it
//! makes what is missing and leaves what is there). The bash script of the same name with the same options, output, files,
//! calls and exit codes; its help is src/doc/labyrinth-ecosystem-setup.txt.
//!
//! systemctl, pacman, setfacl, ssh-keygen and ollama are still those programs. The settings files are written here; Sarina's
//! token comes from the kernel's random numbers (the script asked openssl) and its file is made 0640 and Sarina's group's from
//! the start; the image's SHA-256 is computed here (sha256sum).
//!
//! Fixed in both (script and port):
//!   - --help printed four lines of the script's code after its text (the range was 2-19, the text ends at 15);
//!   - the model's name went into Sarina's service.env unchecked (a newline in it would have added a setting of its own); it must
//!     be a model's name now (^[A-Za-z0-9._:/-]{1,128}$);
//!   - a failed `openssl rand` inside the here-document wrote an empty SARINA_SERVICE_TOKEN; the image's hash and the key's
//!     public half likewise could be left empty in the validation profile. Each is read first now, and a failure stops;
//!   - "is the model there" ran `ollama list | grep -q` under pipefail: when grep stopped reading early, ollama's broken pipe
//!     made it say the model was missing. The list is read whole now (one `ollama list` instead of two).
//!
//! Not 1:1: `--model` without a value is a usage error (exit code 2; bash stopped on the unbound $2, exit code 1); the key's
//! public half and the image path are written into the profile as JSON strings (the same text for what they hold).

use crate::sh::{self, Io};
use std::io::Read;

pub const DOC: &str = include_str!("doc/labyrinth-ecosystem-setup.txt");
const HEIMDALL_CONFIG: &str = include_str!("text/heimdall-config.json");
const OLLAMA_CONF: &str = "# Sarina needs at least 8k tokens of context (16k recommended); Ollama's default is 4k.\n[Service]\nEnvironment=OLLAMA_CONTEXT_LENGTH=16384\n";
const SARINA_DROPIN: &str = "# Sarina talks to Limbo's agent socket (group limbo-agent) and asks Heimdall to review memory writes.\n[Service]\nSupplementaryGroups=limbo-agent\n";
const LIMBO_DROPIN: &str = "# Heimdall may restrict a Limbo guest and read its audit log; the group is the whole access control.\n[Service]\nSupplementaryGroups=limbo-heimdall\nReadOnlyPaths=/var/lib/limbo/state\n";
const CONNWATCH_DROPIN: &str =
    "[Service]\nAmbientCapabilities=CAP_SYS_PTRACE CAP_DAC_READ_SEARCH\nCapabilityBoundingSet=CAP_SYS_PTRACE CAP_DAC_READ_SEARCH\n";
const IMG: &str = "/var/lib/limbo/images/arch-cloudimg.qcow2";

type Res = Result<(), i32>;

fn say(m: &str) {
    println!("==> {m}");
}

fn fail(m: &str) -> i32 {
    eprintln!("labyrinth-ecosystem-setup: {m}");
    1
}

fn have_unit(u: &str) -> bool {
    sh::quiet(&["systemctl", "cat", u])
}

/// ^[A-Za-z0-9._:/-]{1,128}$
pub fn model_ok(m: &str) -> bool {
    (1..=128).contains(&m.len()) && m.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'/' | b'-'))
}

fn install_dir(p: &str, mode: u32, owner: Option<(&str, &str)>) -> Res {
    let ids = match owner {
        None => None,
        Some((u, g)) => {
            let uid = if u == "root" { Some(0) } else { sh::user_id(u).map(|x| x.0) };
            let Some(uid) = uid else { return Err(fail(&format!("install: invalid user '{u}'"))) };
            let Some(gid) = sh::group_id(g) else { return Err(fail(&format!("install: invalid group '{g}'"))) };
            Some((uid, gid))
        }
    };
    sh::install_dir(p, mode, ids).map_err(|e| fail(&format!("install: cannot make {p}: {e}")))
}

/// `[[ -f P ]] || cat > P`
fn write_once(p: &str, text: &str) -> Res {
    if sh::is_file(p) {
        return Ok(());
    }
    sh::write(p, text.as_bytes()).map_err(|e| fail(&format!("cannot write {p}: {e}")))
}

pub fn service_env(token: &str, model: &str) -> String {
    format!(
        "# Written by labyrinth-ecosystem-setup. Sarina's service: a local Ollama model, an access token, Limbo and Heimdall.\n\
         SARINA_SERVICE_TOKEN={token}\n\
         OPENAI_BASE_URL=http://127.0.0.1:11434/v1\n\
         OPENAI_API_KEY=local-token\n\
         OPENAI_MODEL={model}\n\
         SARINA_LIMBO_SOCKET=/run/limbo/agent.sock\n\
         HEIMDALL_REVIEW_URL=http://127.0.0.1:8930/v1/review\n\
         SARINA_TEMPERATURE=0.6\n\
         SARINA_REQUEST_MAX_TOKENS=6000\n\
         SARINA_SEMANTIC=on\n\
         # the cryptography and steganography tools, offered for questions about them (on: always; off: never)\n\
         SARINA_CRYPTO=auto\n"
    )
}

fn json_str(s: &str) -> String {
    let mut out = String::new();
    crate::pycompat::json::string(&mut out, s);
    out
}

pub fn profile(sha: &str, public_key: &str) -> String {
    format!(
        "{{\n  \"manifest\": {{\n    \"name\": \"validate\", \"os\": \"linux\",\n    \"image\": {{\"path\": {}, \"sha256\": {}}},\n    \
         \"cpus\": 2, \"memory_mb\": 2048,\n    \"network\": {{\"mode\": \"none\"}},\n    \"disposable\": true,\n    \
         \"access\": {{\"user\": \"analyst\", \"sudo\": true}},\n    \"app\": {{\"name\": \"validate\", \"exec\": \"true\"}},\n    \
         \"provision\": {{\"ssh_public_key\": {}}}\n  }},\n  \"ssh_identity\": \"/etc/limbo/keys/validation\",\n  \"max_seconds\": 600,\n  \
         \"max_output_bytes\": 65536\n}}\n",
        json_str(IMG),
        json_str(sha),
        json_str(public_key)
    )
}

fn sha256_file(p: &str) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// grep -q "^PATTERN": a line starting with the pattern, its `.` any one character (the rest is plain in a model's name).
pub fn listed(list: &str, pattern: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    list.split('\n').any(|line| {
        let l: Vec<char> = line.chars().collect();
        l.len() >= p.len() && p.iter().zip(&l).all(|(a, b)| *a == '.' || a == b)
    })
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-ecosystem-setup");
    let (mut model, mut profile_on) = ("qwen3:8b".to_string(), true);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--model needs a value");
                    return 2;
                };
                model = v.clone();
                i += 1;
            }
            "--no-limbo-profile" => profile_on = false,
            "-h" | "--help" => {
                print!("{DOC}");
                return 0;
            }
            o => {
                eprintln!("unknown option {o}");
                return 2;
            }
        }
        i += 1;
    }
    if sh::euid() != 0 {
        eprintln!("run as root: sudo labyrinth-ecosystem-setup");
        return 1;
    }
    if !model_ok(&model) {
        eprintln!("bad model name");
        return 2;
    }
    match run(&model, profile_on) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn run(model: &str, profile_on: bool) -> Res {
    // Ollama: Labyrinth's settings come with labyrinth-ollama-tuning (a drop-in that follows Ollama's updates), else once here
    if have_unit("ollama.service") {
        if sh::quiet(&["pacman", "-Q", "labyrinth-ollama-tuning"]) {
            say("ollama: settings come from labyrinth-ollama-tuning");
        } else {
            install_dir("/etc/systemd/system/ollama.service.d", 0o755, None)?;
            write_once("/etc/systemd/system/ollama.service.d/labyrinth.conf", OLLAMA_CONF)?;
            say("ollama: context 16384 (install labyrinth-ollama-tuning to have this kept with updates)");
        }
    } else {
        say("ollama is not installed (pacman -S ollama, or ollama-cuda on an NVIDIA machine): skipping");
    }

    // Sarina
    if have_unit("sarina-service.service") {
        install_dir("/etc/sarina", 0o750, Some(("root", "sarina")))?;
        let env = "/etc/sarina/service.env";
        if !sh::is_file(env) {
            let gid = sh::group_id("sarina").ok_or_else(|| fail("chown: invalid group: 'root:sarina'"))?;
            let text = service_env(&sh::random_hex(24), model);
            sh::replace_file(env, text.as_bytes(), 0o640, Some((0, gid))).map_err(|e| fail(&format!("cannot write {env}: {e}")))?;
            say("sarina: wrote /etc/sarina/service.env (new token)");
        }
        install_dir("/etc/systemd/system/sarina-service.service.d", 0o755, None)?;
        write_once("/etc/systemd/system/sarina-service.service.d/ecosystem.conf", SARINA_DROPIN)?;
        install_dir("/var/lib/sarina/sessions", 0o755, Some(("sarina", "sarina")))?;
        if sh::have("setfacl") {
            sh::run(&["setfacl", "-m", "u:heimdall:rX,g:wheel:rwx", "/var/lib/sarina/sessions"], Io::Inherit, Io::Null);
            sh::run(&["setfacl", "-d", "-m", "u:heimdall:rX,u:sarina:rwx,g:wheel:rwx", "/var/lib/sarina/sessions"], Io::Inherit, Io::Null);
        }
    }

    // Heimdall
    if have_unit("heimdall.service") {
        install_dir("/etc/heimdall", 0o755, None)?;
        install_dir("/etc/systemd/system/heimdall.service.d", 0o755, None)?;
        if !sh::is_file("/etc/heimdall/config.json") {
            write_once("/etc/heimdall/config.json", HEIMDALL_CONFIG)?;
            say("heimdall: wrote /etc/heimdall/config.json");
        }
        write_once("/etc/systemd/system/heimdall.service.d/limbo.conf", LIMBO_DROPIN)?;
        // the connection log reads other users' /proc entries; heimdall.service's sandbox keeps these capabilities to reading
        write_once("/etc/systemd/system/heimdall.service.d/connwatch.conf", CONNWATCH_DROPIN)?;
    }

    // Limbo: a validation profile (a disposable guest with no network) when there is a base image
    if have_unit("limbo.service") && profile_on {
        let prof = "/etc/limbo/validation/arch-validate.json";
        if sh::is_file(IMG) && !sh::is_file(prof) {
            install_dir("/etc/limbo/validation", 0o755, None)?;
            install_dir("/etc/limbo/keys", 0o700, None)?;
            let key = "/etc/limbo/keys/validation";
            if !sh::is_file(key) {
                let code = sh::status(&["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "validation@labyrinth", "-f", key]);
                if code != 0 {
                    return Err(code);
                }
            }
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o600)).map_err(|e| fail(&format!("chmod: {key}: {e}")))?;
            let sha = sha256_file(IMG).map_err(|e| fail(&format!("cannot read {IMG}: {e}")))?;
            let public = sh::cat(&format!("{key}.pub")).ok_or_else(|| fail(&format!("cannot read {key}.pub")))?;
            sh::write(prof, profile(&sha, &public).as_bytes()).map_err(|e| fail(&format!("cannot write {prof}: {e}")))?;
            std::fs::set_permissions(prof, std::fs::Permissions::from_mode(0o644)).map_err(|e| fail(&format!("chmod: {prof}: {e}")))?;
            say("limbo: wrote the validation profile arch-validate (disposable guest, no network)");
        } else if !sh::is_file(IMG) {
            say(&format!("limbo: no base image at {IMG}, so no validation profile (see Limbo's docs for the Arch cloud image)"));
        }
    }

    let code = sh::status(&["systemctl", "daemon-reload"]);
    if code != 0 {
        return Err(code);
    }
    for u in ["ollama", "limbo", "heimdall", "sarina-service"] {
        let unit = format!("{u}.service");
        if have_unit(&unit) {
            if sh::quiet(&["systemctl", "enable", "--now", &unit]) {
                say(&format!("enabled and started {u}"));
            } else {
                eprintln!("warning: could not start {u}");
            }
        }
    }
    if sh::have("ollama") {
        let (_, out) = sh::capture(&["ollama", "list"], Io::Null);
        let models = String::from_utf8_lossy(&out);
        if !listed(&models, model) {
            println!();
            println!("The model {model} is not on this machine yet. Pull it (5 GB):  ollama pull {model}");
        }
        if !listed(&models, "nomic-embed-text") {
            println!("For finding her notes by meaning, also pull the small embedding model (270 MB):  ollama pull nomic-embed-text");
        }
    }
    say("done. Check it with: curl -s 127.0.0.1:8930/healthz ; labyrinth-ask-sarina 'hello'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models() {
        for ok in ["qwen3:8b", "hf.co/user/model:Q4_K_M", "nomic-embed-text"] {
            assert!(model_ok(ok));
        }
        for bad in ["", "a b", "a\nSARINA_SERVICE_TOKEN=x", "a;b", "é", &"a".repeat(129)] {
            assert!(!model_ok(bad));
        }
        assert!(listed("NAME ID\nqwen3:8b  abc  5 GB\n", "qwen3:8b"));
        assert!(!listed("NAME ID\nqwen3:4b  abc\n", "qwen3:8b"));
        assert!(listed("llama3x1\n", "llama3.1"));
        assert!(!listed("", "x"));
    }

    #[test]
    fn texts() {
        let p = profile("ab", "ssh-ed25519 AAAA validation@labyrinth");
        assert!(p.contains("\"image\": {\"path\": \"/var/lib/limbo/images/arch-cloudimg.qcow2\", \"sha256\": \"ab\"},\n"));
        assert!(p.contains("\"provision\": {\"ssh_public_key\": \"ssh-ed25519 AAAA validation@labyrinth\"}\n"));
        assert!(service_env("t", "m").contains("\nSARINA_SERVICE_TOKEN=t\n") && service_env("t", "m").contains("\nOPENAI_MODEL=m\n"));
    }
}
