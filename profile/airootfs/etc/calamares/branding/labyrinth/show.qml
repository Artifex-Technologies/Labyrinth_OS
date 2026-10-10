/* Labyrinth OS installer slideshow: plain text slides. */
import QtQuick 2.0
import calamares.slideshow 1.0

Presentation {
    id: presentation

    Timer {
        interval: 8000
        running: true
        repeat: true
        onTriggered: presentation.goToNextSlide()
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e1e16" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6f4ea"
            font.pixelSize: 22
            text: "Welcome to Labyrinth OS

Built on Arch and CachyOS, with a Hyprland desktop."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e1e16" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6f4ea"
            font.pixelSize: 22
            text: "Snapshots

Btrfs snapshots appear in the boot menu, so a bad update is one reboot from undone."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e1e16" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6f4ea"
            font.pixelSize: 22
            text: "Games

Steam and Proton are set up for your NVIDIA card."
        }
    }

    Slide {
        Rectangle { anchors.fill: parent; color: "#0e1e16" }
        Text {
            anchors.centerIn: parent
            width: parent.width * 0.8
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: "#e6f4ea"
            font.pixelSize: 22
            text: "Your files

Finder previews almost anything and opens ZIP, 7z and RAR like folders."
        }
    }
}
