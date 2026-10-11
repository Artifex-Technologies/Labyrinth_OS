// Labyrinth macOS-style Plasma layout, applied once per user by labyrinth-plasma-setup.
// Top bar (like macOS, without a logo): desktops and the focused app's menus (File, Edit, View...)
// on the left; the tray (media controls, network, volume, Bluetooth), clock and power menu on the right.
// Dock: a floating, centred panel of app icons along the bottom. A full-screen window covers both.
// API: https://develop.kde.org/docs/plasma/scripting/ (Plasma 6 desktop scripting).

panels().forEach(function (p) { p.remove(); });

var top = new Panel;
top.location = "top";
top.height = 2 * Math.floor(gridUnit * 1.6 / 2);
top.floating = false;
top.hiding = "none";

top.addWidget("org.kde.plasma.pager");           // desktops: switch and add
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

// The launcher lives on the dock, with the Labyrinth logo. Tapping Meta still opens it.
var launcher = dock.addWidget("org.kde.plasma.kickoff");
launcher.currentConfigGroup = ["General"];
launcher.writeConfig("icon", "labyrinthos");
launcher.currentConfigGroup = ["Shortcuts"];
launcher.writeConfig("global", "Alt+F1");

var tasks = dock.addWidget("org.kde.plasma.icontasks");
tasks.currentConfigGroup = ["General"];
tasks.writeConfig("launchers", [
    "applications:finder.desktop",
    "applications:vivaldi-stable.desktop",
    "applications:org.kde.konsole.desktop",
    "applications:obsidian.desktop",
    "applications:spotify-launcher.desktop",
    "applications:steam.desktop",
    "applications:systemsettings.desktop"
]);
