// The Omarchy-style coloured border around the focused window on the tiling screen.
//
// Tiled windows have no title bar or frame (Krohnkite's noTileBorder), so this marks the active
// one with a thin accent-coloured line drawn in the gap just outside it: four strips, so nothing
// ever sits over the app, and they take no input (outputOnly). Only on the outputs listed in kwinrc
// [Script-labyrinth-tile-border] tilingOutputs (written by labyrinth-tiling); hidden for full-screen
// and minimized windows. The colour is Labyrinth's accent; kwinrc [Script-labyrinth-tile-border]
// colour=#rrggbb changes it (the theme's accent is not available to KWin scripts: it read as black).
//
// The strips are Plasma on-screen-display dialogs, as Krohnkite's popups are: plain QtQuick
// Windows declared in a KWin script were never shown (seen in the VM, 2026-10-11). KWin caches
// compiled QML in ~/.cache/kwin/qmlcache across logins; clear it when editing this file.
import QtQuick
import org.kde.kwin 3.0
import org.kde.plasma.core as PlasmaCore

Item {
    id: root

    readonly property var tilingOutputs: String(KWin.readConfig("tilingOutputs", ""))
        .split(",").map(function (s) { return s.trim(); }).filter(function (s) { return s.length > 0; })
    readonly property int thickness: KWin.readConfig("thickness", 2)
    readonly property color colour: KWin.readConfig("colour", "#7c54d6")

    readonly property var win: Workspace.activeWindow
    readonly property bool shown: !!win && win.normalWindow && !win.desktopWindow && !win.fullScreen && !win.minimized
        && !!win.output && tilingOutputs.indexOf(win.output.name) >= 0
        && win.frameGeometry.width > 0
    readonly property rect g: shown ? win.frameGeometry : Qt.rect(0, 0, 1, 1)

    // Four thin strips around the window, in the gap Krohnkite leaves, so nothing covers the app.
    component Strip: PlasmaCore.Dialog {
        property int stripWidth: 1
        property int stripHeight: 1
        type: PlasmaCore.Dialog.OnScreenDisplay
        flags: Qt.WindowStaysOnTopHint | Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus
        location: PlasmaCore.Types.Floating
        backgroundHints: PlasmaCore.Dialog.NoBackground
        outputOnly: true
        visible: root.shown
        // The dialog takes its size from the item's implicit size; plain width/height left it 1x1.
        mainItem: Rectangle {
            implicitWidth: stripWidth
            implicitHeight: stripHeight
            width: stripWidth
            height: stripHeight
            color: root.colour
        }
    }

    Strip { x: root.g.x - root.thickness; y: root.g.y - root.thickness
            stripWidth: root.g.width + 2 * root.thickness; stripHeight: root.thickness }
    Strip { x: root.g.x - root.thickness; y: root.g.y + root.g.height
            stripWidth: root.g.width + 2 * root.thickness; stripHeight: root.thickness }
    Strip { x: root.g.x - root.thickness; y: root.g.y
            stripWidth: root.thickness; stripHeight: root.g.height }
    Strip { x: root.g.x + root.g.width; y: root.g.y
            stripWidth: root.thickness; stripHeight: root.g.height }
}
