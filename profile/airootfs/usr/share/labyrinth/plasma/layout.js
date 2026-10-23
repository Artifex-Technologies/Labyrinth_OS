// Labyrinth macOS-style Plasma layout, applied once per user by labyrinth-plasma-setup.
// Top bar (like macOS): the Labyrinth logo as the launcher button, desktops and the focused app's menus
// (File, Edit, View...) on the left; the tray (media controls, network, volume, Bluetooth), clock and power menu on the right.
// Dock: a floating, centred panel of app icons along the bottom. A full-screen window covers both.
// API: https://develop.kde.org/docs/plasma/scripting/ (Plasma 6 desktop scripting).

panels().forEach(function (p) { p.remove(); });

var top = new Panel;
top.location = "top";
top.height = 2 * Math.floor(gridUnit * 1.6 / 2);
top.floating = false;
top.hiding = "none";

// The launcher button at the left end, as the Apple menu sits on a Mac, but with the Labyrinth logo.
var launcher = top.addWidget("org.kde.plasma.kickoff");
launcher.currentConfigGroup = ["General"];
launcher.writeConfig("icon", "labyrinthos");
launcher.currentConfigGroup = ["Shortcuts"];
launcher.writeConfig("global", "Alt+F1");
// The focused app's name in bold, like macOS. It is there for apps whose menus cannot be shown in
// the bar (Vivaldi, Obsidian and other Electron apps), and the menus follow it for the rest.
var appname = top.addWidget("org.kde.windowtitle");
appname.currentConfigGroup = ["General"];
appname.writeConfig("txt", "%a");
appname.writeConfig("altTxt", "");
appname.writeConfig("txtSameFound", "%a");
appname.writeConfig("isBold", true);
appname.writeConfig("noIcon", true);
top.addWidget("org.kde.plasma.appmenu");         // the focused app's File/Edit/View menu
top.addWidget("org.kde.plasma.panelspacer");
top.addWidget("org.kde.plasma.systemtray");      // media controller, network, volume, Bluetooth, battery
top.addWidget("org.kde.plasma.digitalclock");
top.addWidget("org.kde.plasma.lock_logout");     // lock, sleep, hibernate, restart, shut down

var dock = new Panel;
dock.location = "bottom";
dock.height = 64;
dock.lengthMode = "fit";
dock.alignment = "center";
dock.floating = true;
dock.hiding = "none";

var tasks = dock.addWidget("org.kde.plasma.icontasks");
tasks.currentConfigGroup = ["General"];
tasks.writeConfig("launchers", [
    "applications:minos.desktop",
    "applications:vivaldi-stable.desktop",
    "applications:com.mitchellh.ghostty.desktop",
    "applications:obsidian.desktop",
    "applications:spotify-launcher.desktop",
    "applications:steam.desktop",
    "applications:yggdrasil-settings.desktop"
]);
// The trash, at the right end: empty or full by what is in it (Plasma's own widget; it opens
// trash:/ , which Minos understands).
dock.addWidget("org.kde.plasma.trash");
