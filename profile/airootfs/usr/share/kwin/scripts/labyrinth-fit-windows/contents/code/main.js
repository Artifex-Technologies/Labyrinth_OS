// Keep windows out from under the dock and top bar, as Windows and macOS do.
//
// KWin stops a maximized window at the dock, but an app that opens (or later grows) taller than
// the space left was placed with its bottom underneath the dock: the Finder at its default size on
// a small screen, the installer's summary page. This shrinks such a window to the free area and
// moves it inside. Full-screen windows are left alone (they cover the dock on purpose; Meta+Space
// shows it), and so is a window the user is moving or resizing.

function fit(w, reposition) {
    if (!w.normalWindow || w.fullScreen || w.move || w.resize) {
        return;
    }
    var area = workspace.clientArea(KWin.MaximizeArea, w);
    var g = w.frameGeometry;
    var width = w.resizeable ? Math.min(g.width, area.width) : g.width;
    var height = w.resizeable ? Math.min(g.height, area.height) : g.height;
    var x = g.x, y = g.y;
    if (reposition || width !== g.width || height !== g.height) {
        x = Math.max(area.x, Math.min(g.x, area.x + area.width - width));
        y = Math.max(area.y, Math.min(g.y, area.y + area.height - height));
    }
    if (x !== g.x || y !== g.y || width !== g.width || height !== g.height) {
        // A plain object: changing the fields of g and assigning it back is silently ignored.
        w.frameGeometry = {x: x, y: y, width: width, height: height};
    }
}

function watch(w) {
    fit(w, true);
    // An app that grows after it opens (a taller page in a wizard) is fitted again; a window the
    // user drags partly off screen is not pulled back.
    w.frameGeometryChanged.connect(function () { fit(w, false); });
}

workspace.windowList().forEach(watch);
workspace.windowAdded.connect(watch);
