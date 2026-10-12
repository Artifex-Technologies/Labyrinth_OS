// One set of keys, two behaviours: the Windows-style shortcut on the everyday screen and the
// Omarchy-style one on the tiling screen (the screen Krohnkite tiles; see labyrinth-tiling).
//
// Which screen counts is the active window's, or the screen under the pointer when no window is
// focused. The tiling outputs come from kwinrc [Script-labyrinth-screen-keys] tilingOutputs
// (connector names such as HDMI-A-1, comma separated), written by labyrinth-tiling. With none set,
// every key does the Windows thing, so a one-monitor PC is unaffected.
//
// The built-in shortcuts these keys replace (quick tile, overview, zoom, task manager entries...)
// are unbound in kglobalshortcutsrc and called here by name, so they still work on their screen.

var tilingOutputs = String(readConfig("tilingOutputs", "")).split(",")
    .map(function (s) { return s.trim(); })
    .filter(function (s) { return s.length > 0; });

function isAppWindow(w) {
    return !!w && !!w.output && !w.desktopWindow && !w.dock;
}

// The screen you were last working on. Switching the tiling screen to an empty workspace leaves no
// focused window; without this the keys would follow the mouse to the other screen.
var lastOutput = null;
workspace.windowActivated.connect(function (w) {
    if (isAppWindow(w)) {
        lastOutput = w.output;
    }
});

function currentOutput() {
    var w = workspace.activeWindow;
    if (isAppWindow(w)) {
        return w.output;
    }
    return lastOutput || workspace.activeScreen;
}

function onTilingScreen() {
    var o = currentOutput();
    return !!o && tilingOutputs.indexOf(o.name) >= 0;
}

function invoke(component, action) {
    callDBus("org.kde.kglobalaccel", "/component/" + component, "org.kde.kglobalaccel.Component",
             "invokeShortcut", action);
}

function krohnkite(action) { invoke("kwin", "Krohnkite" + action); }

function route(id, text, key, tiling, main) {
    registerShortcut("Labyrinth " + id, "Labyrinth: " + text, key, function () {
        if (onTilingScreen()) {
            tiling();
        } else if (main) {
            main();
        }
    });
}

function active() { return workspace.activeWindow; }

// Win + Arrows: snap like Windows / move focus between tiles.
route("Left", "Snap left / focus left", "Meta+Left",
      function () { krohnkite("FocusLeft"); }, function () { workspace.slotWindowQuickTileLeft(); });
route("Right", "Snap right / focus right", "Meta+Right",
      function () { krohnkite("FocusRight"); }, function () { workspace.slotWindowQuickTileRight(); });
route("Up", "Maximize / focus up", "Meta+Up",
      function () { krohnkite("FocusUp"); },
      function () { var w = active(); if (w && w.maximizable) { w.setMaximize(true, true); } });
route("Down", "Restore or minimize / focus down", "Meta+Down",
      function () { krohnkite("FocusDown"); },
      function () {
          var w = active();
          if (!w) { return; }
          var a = workspace.clientArea(KWin.MaximizeArea, w);
          var g = w.frameGeometry;
          if (g.width >= a.width && g.height >= a.height) {
              w.setMaximize(false, false);
          } else if (w.minimizable) {
              w.minimized = true;
          }
      });

// Win + Shift + Arrows: move the window to the other monitor / swap tiles.
route("ShiftLeft", "Window to left monitor / swap left", "Meta+Shift+Left",
      function () { krohnkite("ShiftLeft"); }, function () { workspace.slotWindowToPrevScreen(); });
route("ShiftRight", "Window to right monitor / swap right", "Meta+Shift+Right",
      function () { krohnkite("ShiftRight"); }, function () { workspace.slotWindowToNextScreen(); });
// Win + Shift + Alt + Left/Right: the window to the other monitor from either screen (on the
// tiling screen Win + Shift + Arrows swaps tiles instead).
function toPrev() { workspace.slotWindowToPrevScreen(); }
function toNext() { workspace.slotWindowToNextScreen(); }
route("AltShiftLeft", "Window to left monitor", "Meta+Shift+Alt+Left", toPrev, toPrev);
route("AltShiftRight", "Window to right monitor", "Meta+Shift+Alt+Right", toNext, toNext);
route("ShiftUp", "Swap up (tiling screen)", "Meta+Shift+Up",
      function () { krohnkite("ShiftUp"); }, null);
route("ShiftDown", "Swap down (tiling screen)", "Meta+Shift+Down",
      function () { krohnkite("ShiftDown"); }, null);

// Win + 1..9: open the dock's app N / switch this screen's workspace. Win + Shift + N sends the
// window to workspace N of the tiling screen.
function desktop(n) {
    var d = workspace.desktops;
    while (d.length < n) {
        workspace.createDesktop(d.length, "Workspace " + (d.length + 1));
        d = workspace.desktops;
    }
    return d[n - 1];
}

[1, 2, 3, 4, 5, 6, 7, 8, 9].forEach(function (n) {
    route("Workspace" + n, "Dock app " + n + " / workspace " + n, "Meta+" + n,
          // (desktop, output): the other order throws "incompatible arguments".
          function () { workspace.setCurrentDesktopForScreen(desktop(n), currentOutput()); },
          function () { invoke("plasmashell", "activate task manager entry " + n); });
    route("ToWorkspace" + n, "Window to workspace " + n + " (tiling screen)", "Meta+Shift+" + n,
          function () { var w = active(); if (w) { w.desktops = [desktop(n)]; } }, null);
});

// Single keys that mean something else in each world.
route("W", "Overview / close window", "Meta+W",
      function () { var w = active(); if (w && w.closeable) { w.closeWindow(); } },
      function () { invoke("kwin", "Overview"); });
route("Space", "Show dock and top bar / launcher", "Meta+Space",
      function () { callDBus("org.kde.krunner", "/App", "org.kde.krunner.App", "display"); },
      function () { invoke("plasmashell", "cycle-panels"); });
route("T", "Tile editor / float or tile window", "Meta+T",
      function () { krohnkite("ToggleFloat"); }, function () { invoke("kwin", "Edit Tiles"); });
route("Minus", "Zoom out / shrink tile", "Meta+-",
      function () { krohnkite("ShrinkWidth"); }, function () { invoke("kwin", "view_zoom_out"); });
route("Equal", "Zoom in / grow tile", "Meta+=",
      function () { krohnkite("growWidth"); }, function () { invoke("kwin", "view_zoom_in"); });
route("ShiftMinus", "Shrink tile height (tiling screen)", "Meta+Shift+-",
      function () { krohnkite("ShrinkHeight"); }, null);
route("ShiftEqual", "Grow tile height (tiling screen)", "Meta+Shift+=",
      function () { krohnkite("GrowHeight"); }, null);
