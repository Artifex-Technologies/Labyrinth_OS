import sys, os, importlib.machinery, importlib.util
p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-appnet")
l = importlib.machinery.SourceFileLoader("an", p); spec = importlib.util.spec_from_loader("an", l); an = importlib.util.module_from_spec(spec); l.exec_module(an)
cg = open("/proc/self/cgroup").read().strip().split("::")[1].strip("/")
an.cgroup_path = lambda uid, app: cg
allowed = sys.argv[1:]
ask = {"1000": {"web": {"allow": {ip: "" for ip in allowed}, "deny": {}}}}
print(an.ruleset({"1000": {"web": "ask"}}, exists=lambda p: True, ask=ask))
