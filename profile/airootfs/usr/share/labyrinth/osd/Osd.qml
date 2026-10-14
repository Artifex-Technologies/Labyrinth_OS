/*
    Labyrinth's on-screen message: a black rounded pill. (Volume and brightness are drawn by the panel.)
    Same interface as Plasma's own osd/Osd.qml (OsdWindow with a main item), which it replaces.
*/

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import org.kde.plasma.workspace.osd

OsdWindow {
    id: window

    LayoutMirroring.enabled: Application.layoutDirection === Qt.RightToLeft
    LayoutMirroring.childrenInherit: true

    property alias timeout: osd.timeout
    property alias osdValue: osd.osdValue
    property alias osdMaxValue: osd.osdMaxValue
    property alias osdAdditionalText: osd.osdAdditionalText
    property alias icon: osd.icon
    property alias showingProgress: osd.showingProgress

    width: 300 + leftPadding + rightPadding
    height: osd.implicitHeight + topPadding + bottomPadding

    // Volume and brightness are shown by the panel (a little above the dock, where KWin would not
    // put this window); this one only draws the other messages, such as the keyboard layout.
    readonly property bool shownByPanel: /volume|brightness/i.test(String(osd.icon))
    opacity: shownByPanel ? 0 : 1

    mainItem: Item {
        id: osd

        property int timeout: 1800
        property var osdValue
        property int osdMaxValue: 100
        property string osdAdditionalText: ""
        property string icon
        property bool showingProgress: false

        readonly property real level: Math.max(0, Math.min(1, Number(osdValue) / osdMaxValue))

        implicitWidth: 300
        implicitHeight: 54

        // the pill covers the whole window, including the padding Plasma's own frame is drawn in
        Rectangle {
            x: -window.leftPadding
            y: -window.topPadding
            width: window.width
            height: window.height
            radius: 10
            color: "#000000"
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 16
            anchors.rightMargin: 16
            spacing: 12

            Kirigami.Icon {
                Layout.preferredWidth: 22
                Layout.preferredHeight: 22
                Layout.alignment: Qt.AlignVCenter
                source: osd.icon
                color: "#ffffff"
                isMask: true
                visible: valid
            }

            // the level, as a track with the accent colour filling it
            Rectangle {
                visible: osd.showingProgress
                Layout.fillWidth: true
                Layout.preferredHeight: 6
                Layout.alignment: Qt.AlignVCenter
                radius: 3
                color: Qt.rgba(1, 1, 1, 0.14)

                Rectangle {
                    width: parent.width * osd.level
                    height: parent.height
                    radius: 3
                    color: Kirigami.Theme.highlightColor
                }
            }

            Text {
                visible: osd.showingProgress
                Layout.preferredWidth: 40
                Layout.alignment: Qt.AlignVCenter
                horizontalAlignment: Text.AlignRight
                color: "#ffffff"
                font.pixelSize: 13
                font.bold: true
                text: Math.round(Number(osd.osdValue)) + "%"
            }

            // a message instead of a level (for example the keyboard layout)
            Text {
                visible: !osd.showingProgress
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                color: "#ffffff"
                font.pixelSize: 14
                font.bold: true
                text: osd.osdValue ?? ""
            }
        }
    }
}
