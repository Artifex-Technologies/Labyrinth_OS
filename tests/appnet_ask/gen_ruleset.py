# The Ask rules' nftables text for netns_test.sh, from labyrinth-appnet's ruleset(). The Python script is taken from git (the
# last commit that had it: it was kept in packages/tools/python for a rollback until 2026-11-15); the Rust labyrinth-appnet
# (packages/tools/src/appnet.rs) writes the same text, which its parity cases check.
import os, subprocess, sys, types
REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
src = subprocess.run(["git", "-C", REPO, "show", "5ed56c8:packages/tools/python/labyrinth-appnet"], capture_output=True, check=True).stdout
an = types.ModuleType("an"); sys.modules["an"] = an; exec(compile(src, "labyrinth-appnet", "exec"), an.__dict__)
cg = open("/proc/self/cgroup").read().strip().split("::")[1].strip("/")
an.cgroup_path = lambda uid, app: cg
allowed = sys.argv[1:]
ask = {"1000": {"web": {"allow": {ip: "" for ip in allowed}, "deny": {}}}}
print(an.ruleset({"1000": {"web": "ask"}}, exists=lambda p: True, ask=ask))
