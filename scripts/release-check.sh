#!/usr/bin/env bash
# Everything that can be checked before a release WITHOUT root, a VM or a network install. Prints PASS / WARN / FAIL per step and
# exits 1 if anything FAILed. The two things it cannot do (build the ISO with mkarchiso; install it in a VM) are listed at the end.
#
#   scripts/release-check.sh          all checks
#   scripts/release-check.sh quick    skip the Rust tests and the dependency audit
set -u
export PATH="$HOME/.cargo/bin:$PATH"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
quick=0; [[ ${1:-} == quick ]] && quick=1
fails=0; warns=0
pass() { echo "PASS  $*"; }
warn() { echo "WARN  $*"; warns=$((warns + 1)); }
fail() { echo "FAIL  $*"; fails=$((fails + 1)); }
step() { local name=$1; shift; local out; if out=$("$@" 2>&1); then pass "$name"; else fail "$name"; echo "$out" | tail -15 | sed 's/^/      /'; fi; }

step "package list is generated from packages.base + components" python3 scripts/gen-packages.py --check
step "components.json is valid" python3 -c 'import json,sys; json.load(open("profile/airootfs/usr/share/labyrinth/components.json"))'

# shell and python syntax of everything we ship
bad=0
while IFS= read -r f; do
  head -1 "$f" | grep -q '^#!.*\(ba\)\?sh' && { bash -n "$f" 2>/dev/null || { fail "shell syntax: $f"; bad=1; }; }
  head -1 "$f" | grep -q '^#!.*python' && { python3 -m py_compile "$f" 2>/dev/null || { fail "python syntax: $f"; bad=1; }; }
done < <(find profile/airootfs/usr/local/bin scripts -type f ! -name '*.md')
((bad)) || pass "shell and python scripts parse"
find . -name __pycache__ -prune -exec rm -rf {} + 2>/dev/null

if command -v shellcheck >/dev/null; then
  step "shellcheck (errors only)" bash -c 'find profile/airootfs/usr/local/bin scripts -type f -exec grep -l "^#!.*\(ba\)\?sh" {} + | xargs shellcheck -S error'
else warn "shellcheck is not installed (sudo pacman -S shellcheck)"; fi

# systemd units
unit_bad=0
for u in $(find profile -name '*.service' -o -name '*.timer'); do
  case "$u" in */user/*) mode=--user;; *) mode=--system;; esac
  out=$(systemd-analyze verify $mode "$u" 2>&1 | grep -v -E "Can.t show|'\(man\)'|Failed to (create|load)|not-found|No such file|Unit .* not found|Cannot find|manager.*not|Transport|Failed to connect|is not executable|Command .* (is not|not found)|Failed to prepare") || true
  [[ -z $out ]] || { fail "unit $u"; unit_bad=1; echo "$out" | head -5 | sed 's/^/      /'; }
done
((unit_bad)) || pass "systemd units verify"

# the packaged scripts all exist where profiledef.sh says they are
miss=0
for p in $(grep -o '\["[^"]*"\]' profile/profiledef.sh | tr -d '[]"' ); do
  case "$p" in /etc/shadow|/etc/gshadow|/root) continue;; esac  # made by mkarchiso itself
  [[ -e "profile/airootfs$p" ]] || { echo "      listed in profiledef.sh but missing: $p"; miss=1; }
done
((miss)) && fail "profiledef.sh lists files that are not in airootfs" || pass "every file listed in profiledef.sh exists"

# PKGBUILDs: they parse, and say what they build
for d in packages/pkgbuilds/*/; do
  if (cd "$d" && bash -n PKGBUILD 2>/dev/null && makepkg --printsrcinfo >/dev/null 2>&1); then :; else fail "PKGBUILD does not parse: $d"; fi
done
pass "PKGBUILDs parse"

# python tests
for t in tests/test_*.py; do step "python $t" python3 -W ignore "$t"; done
for t in tests/test_*.sh; do step "shell $t" bash "$t"; done

if ((quick)); then warn "quick mode: Rust tests and the dependency audit were skipped"; else
  for c in packages/*/Cargo.toml; do
    d=$(dirname "$c"); n=$(basename "$d")
    [[ $n == pkgbuilds ]] && continue
    step "cargo test $n" bash -c "cd '$d' && CARGO_TARGET_DIR=\${CARGO_TARGET_DIR:-/tmp/labyrinth-release-target/$n} cargo test --quiet"
  done
  if command -v cargo-audit >/dev/null; then
    for l in packages/*/Cargo.lock; do
      n=$(basename "$(dirname "$l")")
      out=$(cargo audit -f "$l" 2>&1); rc=$?
      if ((rc)); then fail "cargo audit $n (known vulnerable dependency)"; echo "$out" | grep -E "^(Crate|Version|Title|ID|Solution)" | head -12 | sed 's/^/      /'
      elif echo "$out" | grep -q "^warning"; then warn "cargo audit $n: $(echo "$out" | grep -c '^Crate:') unmaintained or yanked crate(s); run: cargo audit -f $l"
      else pass "cargo audit $n"; fi
    done
  else warn "cargo-audit is not installed (cargo install cargo-audit --locked)"; fi
fi

echo
echo "Not done here (need root or a VM): 1) sudo scripts/build-local-packages.sh && sudo scripts/build.sh  (builds the ISO)"
echo "                                   2) boot the ISO in a VM (scripts/vm), run the installer, then run labyrinth-selfcheck"
echo "Result: $fails failed, $warns warning(s)."
exit $((fails > 0))
