# Programs people install for themselves (Claude Code, pipx, cargo install ...) go to ~/.local/bin.
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) PATH="$HOME/.local/bin:$PATH" ;; esac
export PATH
