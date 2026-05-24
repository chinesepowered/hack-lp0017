// QML entry point for the Whistleblower Basecamp app.
//
// Loaded when Basecamp hosts the app natively (vs. the web preview).
// The QML side wraps the same indexing pipeline via FFI:
//
//   - `WhistleblowerCore` is a tiny QObject wrapper around the
//     `whistleblower-core` cdylib. It exposes:
//       envelopeHashHex(cid, title, ...)  → string
//       defaultTopic()                    → string
//
//   - `LogosDelivery` and `LogosStorage` are provided by the Basecamp
//     module loader at startup. The app does NOT instantiate them.
//
// To package: `qmake` + `aqtinstall` (Qt 6.5+) per the logos-basecamp
// build instructions. See ../docs/BUILD-QML.md.

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import io.logos.basecamp 1.0
import io.logos.whistleblower 1.0

ApplicationWindow {
    id: window
    visible: true
    width: 720
    height: 760
    title: "Whistleblower"
    color: "#0e1116"

    WhistleblowerCore { id: core }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16

        Label {
            text: "Whistleblower"
            font.pixelSize: 28
            color: "#e6edf3"
        }
        Label {
            text: "Upload a document. Broadcast it. Anchor it on-chain — or let someone else."
            color: "#8b949e"
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        GroupBox {
            title: "1. Upload & broadcast"
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 8

                FileSelector { id: fileSel }
                TextField { id: titleField; placeholderText: "Title"; Layout.fillWidth: true }
                TextArea  { id: descArea;  placeholderText: "Description"; Layout.preferredHeight: 60; Layout.fillWidth: true }
                TextField { id: tagsField; placeholderText: "Tags (comma-separated)"; Layout.fillWidth: true }

                Button {
                    text: "Upload & broadcast"
                    enabled: fileSel.path !== "" && titleField.text !== ""
                    onClicked: pipeline.run(fileSel.path, titleField.text, descArea.text, tagsField.text)
                }
            }
        }

        GroupBox {
            title: "2. Anchor on-chain (optional)"
            Layout.fillWidth: true
            visible: pipeline.hasResult
            ColumnLayout {
                anchors.fill: parent
                Button { text: "Anchor this document"; onClicked: pipeline.anchorLast() }
                Label  { text: pipeline.anchorResult; color: "#56d364"; wrapMode: Text.Wrap; Layout.fillWidth: true }
            }
        }

        GroupBox {
            title: "Live feed — " + core.defaultTopic()
            Layout.fillWidth: true
            Layout.fillHeight: true
            ListView {
                id: feed
                anchors.fill: parent
                model: pipeline.feedModel
                delegate: Row {
                    spacing: 12
                    Label { text: model.time; color: "#8b949e"; width: 80 }
                    Label { text: model.cidShort; font.family: "monospace"; color: "#e6edf3"; width: 180; elide: Text.ElideMiddle }
                    Label { text: model.title; color: "#e6edf3"; width: 220; elide: Text.ElideRight }
                    Label { text: model.anchored ? "anchored" : "pending"; color: model.anchored ? "#56d364" : "#f0883e" }
                }
            }
        }
    }

    // Pipeline is the QObject that owns Publisher + BatchAnchor state.
    // It is registered from the C++ host as `WhistleblowerPipeline`.
    WhistleblowerPipeline {
        id: pipeline
        delivery: LogosDelivery
        storage:  LogosStorage
        anchor:   LogosLEZ.program("whistleblower_registry")
    }
}
