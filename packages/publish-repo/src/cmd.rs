//! Starting the programs the publish uses (git, makepkg, bsdtar, gzip, vercmp, gpg, repo-add, gh): always by argv, never
//! through a shell. Standard input is passed on as the script passed it; a program's failure stops the publish with one
//! line on standard error (the script stopped with a traceback; both exit with 1).

use crate::py::universal_newlines;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

pub type R<T> = Result<T, String>;

pub struct Cmd {
    argv: Vec<OsString>,
    cwd: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
}

impl Cmd {
    pub fn new<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(argv: I) -> Cmd {
        Cmd { argv: argv.into_iter().map(|a| a.as_ref().to_os_string()).collect(), cwd: None, env: Vec::new() }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Cmd {
        self.argv.push(a.as_ref().to_os_string());
        self
    }

    pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, args: I) -> Cmd {
        self.argv.extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    pub fn cwd(mut self, d: &Path) -> Cmd {
        self.cwd = Some(d.to_path_buf());
        self
    }

    pub fn env(mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> Cmd {
        self.env.push((k.as_ref().to_os_string(), v.as_ref().to_os_string()));
        self
    }

    fn shown(&self) -> String {
        self.argv.iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
    }

    fn command(&self) -> Command {
        let mut c = Command::new(&self.argv[0]);
        c.args(&self.argv[1..]);
        if let Some(d) = &self.cwd {
            c.current_dir(d);
        }
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c.stdin(Stdio::inherit());
        c
    }

    fn cannot(&self, e: std::io::Error) -> String {
        format!("error: cannot run {}: {e}", self.argv[0].to_string_lossy())
    }

    fn failed(&self, st: ExitStatus, stderr: &[u8]) -> String {
        let mut m = format!("error: {} failed ({st})", self.shown());
        let said = String::from_utf8_lossy(stderr);
        let said = said.trim();
        if !said.is_empty() {
            m.push_str(": ");
            m.push_str(&said.replace('\n', " / "));
        }
        m
    }

    /// Runs it with the publish's own output (the journal); stops on failure.
    pub fn run(&self) -> R<()> {
        let st = self.command().status().map_err(|e| self.cannot(e))?;
        if st.success() {
            Ok(())
        } else {
            Err(self.failed(st, b""))
        }
    }

    fn output(&self, input: Option<&[u8]>) -> R<Output> {
        let mut c = self.command();
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.is_some() {
            c.stdin(Stdio::piped());
        }
        let mut child = c.spawn().map_err(|e| self.cannot(e))?;
        let feeder = match (input, child.stdin.take()) {
            (Some(data), Some(mut stdin)) => {
                let data = data.to_vec();
                // a program that stops reading early (gzip on a broken archive) closes the pipe: not an error of ours
                Some(std::thread::spawn(move || {
                    let _ = stdin.write_all(&data);
                }))
            }
            _ => None,
        };
        let mut err = Vec::new();
        let mut stderr = child.stderr.take();
        let err_reader = std::thread::spawn(move || {
            if let Some(s) = stderr.as_mut() {
                let _ = s.read_to_end(&mut err);
            }
            err
        });
        let mut out = Vec::new();
        if let Some(mut s) = child.stdout.take() {
            s.read_to_end(&mut out).map_err(|e| self.cannot(e))?;
        }
        let status = child.wait().map_err(|e| self.cannot(e))?;
        if let Some(f) = feeder {
            let _ = f.join();
        }
        let err = err_reader.join().unwrap_or_default();
        Ok(Output { status, stdout: out, stderr: err })
    }

    /// Whether it succeeded, its output kept from the journal (gpg's checks).
    pub fn quiet_ok(&self) -> R<bool> {
        Ok(self.output(None)?.status.success())
    }

    /// Runs it with its output kept from the journal; stops on failure.
    pub fn quiet(&self) -> R<()> {
        let o = self.output(None)?;
        if o.status.success() {
            Ok(())
        } else {
            Err(self.failed(o.status, &o.stderr))
        }
    }

    /// Its standard output as bytes (with `input` on its standard input); stops on failure.
    pub fn bytes(&self, input: Option<&[u8]>) -> R<Vec<u8>> {
        let o = self.output(input)?;
        if o.status.success() {
            Ok(o.stdout)
        } else {
            Err(self.failed(o.status, &o.stderr))
        }
    }

    /// Its standard output as text, None when it failed.
    pub fn text_or_none(&self) -> R<Option<String>> {
        let o = self.output(None)?;
        if !o.status.success() {
            return Ok(None);
        }
        let t = String::from_utf8(o.stdout).map_err(|_| format!("error: {} wrote something that is not UTF-8", self.shown()))?;
        Ok(Some(universal_newlines(&t)))
    }

    /// Its standard output as text; stops on failure.
    pub fn text(&self) -> R<String> {
        let o = self.output(None)?;
        if !o.status.success() {
            return Err(self.failed(o.status, &o.stderr));
        }
        let t = String::from_utf8(o.stdout).map_err(|_| format!("error: {} wrote something that is not UTF-8", self.shown()))?;
        Ok(universal_newlines(&t))
    }
}
