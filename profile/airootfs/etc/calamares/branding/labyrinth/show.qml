/* Labyrinth OS installer slideshow: plain text slides. */
import QtQuick 2.0
import calamares.slideshow 1.0

Presentation {
    id: presentation

    // The slides sit inside this item; without a background it shows through as a white frame.
    Rectangle { anchors.fill: parent; color: "#0e0f14"; z: -1 }

    Timer {
        interval: 8000
        running: true
        repeat: true
        onTriggered: presentation.goToNextSlide()
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e0f14" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6e7ee"
            font.pixelSize: 22
            text: "Welcome to Labyrinth OS

Built on Arch and CachyOS, with its own desktop, Yggdrasil, and a tiling mode when you want one."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e0f14" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6e7ee"
            font.pixelSize: 22
            text: "Snapshots

Btrfs snapshots appear in the boot menu, so a bad update is one reboot from undone."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e0f14" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6e7ee"
            font.pixelSize: 22
            text: "Games

Steam and Proton are set up for your NVIDIA card."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e0f14" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6e7ee"
            font.pixelSize: 22
            text: "Your files

Minos previews almost anything and opens ZIP, 7z and RAR like folders."
        }
    }
}
