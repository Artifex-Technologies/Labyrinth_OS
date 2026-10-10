#!/usr/bin/env bash
# Runs INSIDE the VM (type `vmtest-hypr` in a terminal there). Prints how hyprctl behaves on this
# Hyprland version, so scripts that call it can be written against facts. Output also goes to ~/vmtest-hypr.out.
exec > >(tee ~/vmtest-hypr.out) 2>&1
echo "hyprland: $(hyprctl version -j 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])' 2>&1)"
echo "--- workspaces before:"; hyprctl workspaces -j | python3 -c 'import json,sys; print([w["id"] for w in json.load(sys.stdin)])'
echo "--- dispatch workspace 3:"; hyprctl dispatch workspace 3; sleep 0.5
echo "active id: $(hyprctl activeworkspace -j | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
echo "--- dispatch workspace empty:"; hyprctl dispatch workspace empty; sleep 0.5
echo "active id: $(hyprctl activeworkspace -j | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
echo "--- lua-style dispatch:"; hyprctl dispatch 'hl.dsp.focus({ workspace = "1" })'; sleep 0.5
echo "active id: $(hyprctl activeworkspace -j | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
echo "--- workspaces after:"; hyprctl workspaces -j | python3 -c 'import json,sys; print([w["id"] for w in json.load(sys.stdin)])'
echo "--- done"
