# With Labyrinth's top bar and dock on (they are, from the first login), the panel draws the volume and brightness pop-up (a little
# above the dock), so Plasma's own for those two is hidden by usr/share/labyrinth/osd/Osd.qml, which
# also gives the other messages the Labyrinth look. Without the panel Plasma's pop-up is left alone.
# Plasma loads its pop-up from a QML module; a module of the same name found earlier on the import
# path wins. The module is assembled here at each login, linking the compiled parts from the system
# so that it always matches the installed Plasma, and adding only our own Osd.qml and qmldir.
sys=/usr/lib/qt6/qml/org/kde/plasma/workspace/osd
if { [ -e "${XDG_CONFIG_HOME:-$HOME/.config}/labyrinth/panel-on" ] || [ ! -e "${XDG_CONFIG_HOME:-$HOME/.config}/labyrinth/plasma-setup-done" ]; } && [ -r "$sys/libplasmashell_osd.so" ] && [ -r /usr/share/labyrinth/osd/Osd.qml ]; then
  d=${XDG_CACHE_HOME:-$HOME/.cache}/labyrinth/qml/org/kde/plasma/workspace/osd
  mkdir -p "$d" &&
  for f in libplasmashell_osd.so plasmashell_osd.qmltypes OsdItem.qml kde-qmlmodule.version; do
    ln -sf "$sys/$f" "$d/$f"
  done &&
  cp -f /usr/share/labyrinth/osd/Osd.qml /usr/share/labyrinth/osd/qmldir "$d/" &&
  export QML_IMPORT_PATH="${XDG_CACHE_HOME:-$HOME/.cache}/labyrinth/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
fi
