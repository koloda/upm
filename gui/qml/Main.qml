import QtQuick
import QtQuick.Controls.Fusion
import io.upm.gui

ApplicationWindow {
    id: window

    width: 1040
    height: 660
    minimumWidth: 760
    minimumHeight: 420
    visible: true
    title: "UPM"

    onClosing: close => {
        if (!view.canClose) {
            close.accepted = false
            view.showCloseWarning()
        }
    }

    MainView {
        id: view
        anchors.fill: parent
        backend: Backend {}
    }
}
