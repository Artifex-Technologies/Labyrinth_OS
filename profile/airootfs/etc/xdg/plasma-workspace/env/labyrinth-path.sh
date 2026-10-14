# The same for the whole desktop session, so a terminal opened from the dock or the start menu finds
# what is in ~/.local/bin (Claude Code, for one) without a login shell.
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) PATH="$HOME/.local/bin:$PATH" ;; esac
export PATH
