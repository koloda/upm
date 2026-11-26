// Увесь вигляд UPM. Про Rust тут нічого немає: усе приходить через `backend`,
// тому в тестах його замінює звичайний QtObject.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Fusion
import QtQuick.Layouts

Page {
    id: root

    required property var backend

    readonly property var sources: backend.sourcesJson ? JSON.parse(backend.sourcesJson) : []
    readonly property var rows: backend.rowsJson ? JSON.parse(backend.rowsJson) : []
    readonly property string currentSource: backend.currentSource
    readonly property string currentTitle: {
        const found = sources.find(source => source.id === currentSource)
        return found ? found.title : currentSource
    }

    // Позначені пакунки: "джерело\u001fключ" -> [джерело, ключ].
    // Об'єкт щоразу замінюється цілком, щоб прив'язки це помітили.
    property var selected: ({})
    readonly property int selectedCount: Object.keys(selected).length
    readonly property var selectableRows: rows.filter(row => row.source !== "manual")
    readonly property int removableSelectedCount:
        selectableRows.filter(row => isSelected(row) && row.removable).length
    readonly property bool allVisibleSelected:
        selectableRows.length > 0 && selectableRows.every(row => isSelected(row))

    // Під час оновлення закривати вікно не можна: це обірвало б транзакцію.
    readonly property bool canClose: !backend.upgrading
    property bool logShown: false
    property string closeWarning: ""

    onCurrentSourceChanged: selected = ({})

    function rowId(row) { return row.source + "\u001f" + row.key }
    function isSelected(row) { return selected[rowId(row)] !== undefined }
    function selectedJson() { return JSON.stringify(Object.values(selected)) }

    function setSelected(list, on) {
        const next = Object.assign({}, selected)
        for (const row of list) {
            if (on)
                next[rowId(row)] = [row.source, row.key]
            else
                delete next[rowId(row)]
        }
        selected = next
    }

    function toggle(row) { setSelected([row], !isSelected(row)) }
    function toggleAll() { setSelected(selectableRows, !allVisibleSelected) }

    function latestText(row) {
        switch (row.state) {
        case "upgradable": return row.latest
        case "current": return "актуальний"
        case "unknown": return row.source === "manual" ? "вручну" : "не перевіряється"
        default: return ""
        }
    }

    function askUpgrade() {
        confirmDialog.deleteMode = false
        confirmDialog.selection = selectedJson()
        confirmDialog.plan = JSON.parse(backend.planUpgrade(confirmDialog.selection))
        confirmDialog.open()
    }

    function askDelete() {
        confirmDialog.deleteMode = true
        confirmDialog.selection = selectedJson()
        confirmDialog.plan = JSON.parse(backend.planDelete(confirmDialog.selection))
        confirmDialog.open()
    }

    function showCloseWarning() {
        closeWarning = "Дочекайтеся завершення оновлення"
    }

    Connections {
        target: root.backend
        function onUpgradingChanged() {
            if (root.backend.upgrading)
                root.logShown = true
            else
                root.closeWarning = ""
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // ------------------------------------------------------ Джерела
        Pane {
            Layout.fillHeight: true
            Layout.preferredWidth: 210
            padding: 4
            background: Rectangle { color: root.palette.base }

            ListView {
                id: sourceList
                objectName: "sourceList"
                anchors.fill: parent
                clip: true
                model: root.sources
                ScrollBar.vertical: ScrollBar {}

                delegate: ItemDelegate {
                    id: sourceItem
                    required property var modelData

                    width: ListView.view.width
                    highlighted: root.currentSource === modelData.id
                    topPadding: 5
                    bottomPadding: 5
                    onClicked: root.backend.selectSource(modelData.id)

                    contentItem: RowLayout {
                        Label {
                            Layout.fillWidth: true
                            text: sourceItem.modelData.title
                            font.bold: sourceItem.modelData.id === ""
                            elide: Text.ElideRight
                            color: sourceItem.highlighted ? root.palette.highlightedText : root.palette.text
                        }
                        Label {
                            visible: sourceItem.modelData.failed
                            text: "!"
                            font.bold: true
                            color: "#d9822b"
                        }
                        Rectangle {
                            visible: sourceItem.modelData.updates > 0
                            implicitWidth: Math.max(badge.implicitWidth + 10, implicitHeight)
                            implicitHeight: badge.implicitHeight + 2
                            radius: height / 2
                            color: "#2e8b57"
                            Label {
                                id: badge
                                anchors.centerIn: parent
                                text: sourceItem.modelData.updates
                                color: "white"
                                font.pointSize: root.font.pointSize - 1
                            }
                        }
                        Label {
                            text: sourceItem.modelData.installed
                            opacity: 0.6
                            color: sourceItem.highlighted ? root.palette.highlightedText : root.palette.text
                        }
                    }
                }
            }
        }

        // ------------------------------------------------------ Пакунки
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 8
            spacing: 6

            RowLayout {
                TextField {
                    objectName: "searchField"
                    Layout.fillWidth: true
                    placeholderText: "Пошук…"
                    onTextChanged: if (text !== root.backend.search)
                        root.backend.setFilter(root.backend.onlyUpgradable, text)
                }
                CheckBox {
                    objectName: "onlyUpgradable"
                    text: "Лише з оновленнями"
                    checked: root.backend.onlyUpgradable
                    onToggled: {
                        root.backend.setFilter(checked, root.backend.search)
                        // Клік розриває прив'язку, тому відновлюємо її.
                        checked = Qt.binding(() => root.backend.onlyUpgradable)
                    }
                }
                BusyIndicator {
                    running: root.backend.busy
                    visible: running
                    implicitWidth: 28
                    implicitHeight: 28
                }
                Button {
                    objectName: "refreshButton"
                    text: "Refresh"
                    enabled: !root.backend.busy
                    onClicked: root.backend.refresh()
                }
            }

            RowLayout {
                Layout.leftMargin: 6
                Layout.rightMargin: 14
                CheckBox {
                    objectName: "selectAll"
                    enabled: root.selectableRows.length > 0
                    checked: root.allVisibleSelected
                    onClicked: {
                        root.toggleAll()
                        checked = Qt.binding(() => root.allVisibleSelected)
                    }
                }
                Label { Layout.fillWidth: true; text: "Пакунок"; font.bold: true }
                Label { Layout.preferredWidth: 170; text: "Встановлено"; font.bold: true }
                Label { Layout.preferredWidth: 170; text: "Нова"; font.bold: true }
                Label { Layout.preferredWidth: 90; text: "Джерело"; font.bold: true }
            }

            Frame {
                Layout.fillWidth: true
                Layout.fillHeight: true
                padding: 1
                background: Rectangle { color: root.palette.base; border.color: root.palette.mid }

                ListView {
                    id: rowList
                    objectName: "rowList"
                    anchors.fill: parent
                    clip: true
                    model: root.rows
                    ScrollBar.vertical: ScrollBar {}

                    delegate: ItemDelegate {
                        id: rowItem
                        required property var modelData
                        readonly property bool selectable: modelData.source !== "manual"

                        width: ListView.view.width
                        leftPadding: 5
                        rightPadding: 13
                        topPadding: 0
                        bottomPadding: 0
                        onClicked: if (selectable) root.toggle(modelData)

                        highlighted: root.isSelected(modelData)

                        ToolTip.visible: hovered && modelData.info !== ""
                        ToolTip.delay: 600
                        ToolTip.text: modelData.info

                        contentItem: RowLayout {
                            CheckBox {
                                objectName: "rowCheck"
                                padding: 4
                                enabled: rowItem.selectable
                                checked: root.isSelected(rowItem.modelData)
                                onClicked: {
                                    root.toggle(rowItem.modelData)
                                    checked = Qt.binding(() => root.isSelected(rowItem.modelData))
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: rowItem.modelData.name
                                elide: Text.ElideRight
                            }
                            Label {
                                Layout.preferredWidth: 170
                                text: rowItem.modelData.current
                                elide: Text.ElideMiddle
                            }
                            Label {
                                objectName: "rowLatest"
                                Layout.preferredWidth: 170
                                text: root.latestText(rowItem.modelData)
                                elide: Text.ElideMiddle
                                font.bold: rowItem.modelData.state === "upgradable"
                                opacity: rowItem.modelData.state === "upgradable" ? 1 : 0.55
                            }
                            Label {
                                Layout.preferredWidth: 90
                                text: rowItem.modelData.sourceTitle
                                elide: Text.ElideRight
                                opacity: 0.7
                            }
                        }
                    }

                    Label {
                        anchors.centerIn: parent
                        visible: rowList.count === 0 && !root.backend.busy
                        text: root.backend.onlyUpgradable ? "Оновлень немає" : "Нічого не знайдено"
                        opacity: 0.6
                    }
                }
            }

            ScrollView {
                id: logPanel
                objectName: "logPanel"
                visible: root.logShown
                Layout.fillWidth: true
                Layout.preferredHeight: 180

                TextArea {
                    id: logText
                    readOnly: true
                    text: root.backend.log
                    font.family: "monospace"
                    wrapMode: TextEdit.WrapAnywhere
                    background: Rectangle { color: root.palette.base; border.color: root.palette.mid }
                    // Тримаємо в полі зору останній рядок.
                    onTextChanged: cursorPosition = length
                }
            }

            RowLayout {
                Label {
                    objectName: "statusLabel"
                    Layout.fillWidth: true
                    text: root.closeWarning !== "" ? root.closeWarning : root.backend.status
                    font.bold: root.closeWarning !== ""
                    elide: Text.ElideRight
                }
                Button {
                    text: root.logShown ? "Сховати журнал" : "Журнал"
                    flat: true
                    onClicked: root.logShown = !root.logShown
                }
                Button {
                    objectName: "upgradeButton"
                    highlighted: true
                    text: root.selectedCount > 0 ? "Upgrade вибрані (" + root.selectedCount + ")"
                        : root.currentSource !== "" ? "Upgrade усе: " + root.currentTitle
                        : "Upgrade усе"
                    enabled: !root.backend.busy
                        && (root.selectedCount > 0 || root.currentSource !== "manual")
                    onClicked: root.askUpgrade()
                }
                Button {
                    objectName: "deleteSelectedButton"
                    text: root.removableSelectedCount > 0
                        ? "Видалити вибрані (" + root.removableSelectedCount + ")"
                        : "Видалити вибрані"
                    enabled: !root.backend.busy && root.removableSelectedCount > 0
                    onClicked: root.askDelete()
                }
            }
        }
    }

    Dialog {
        id: confirmDialog
        objectName: "confirmDialog"

        property string selection: "[]"
        property bool deleteMode: false
        property var plan: ({ runnable: false, title: "", lines: [] })

        modal: true
        anchors.centerIn: parent
        width: Math.min(root.width - 80, 620)
        height: Math.min(root.height - 80, 460)
        title: plan.title

        onAccepted: {
            if (confirmDialog.deleteMode)
                root.backend.deleteSelected(selection)
            else
                root.backend.upgrade(selection)
            root.selected = ({})
        }

        contentItem: ListView {
            clip: true
            model: confirmDialog.plan.lines
            ScrollBar.vertical: ScrollBar {}
            delegate: Label {
                required property string modelData
                width: ListView.view.width
                text: modelData
                wrapMode: Text.WrapAnywhere
            }
        }

        footer: DialogButtonBox {
            Button {
                objectName: "confirmOk"
                text: confirmDialog.deleteMode ? "Видалити" : "Оновити"
                enabled: confirmDialog.plan.runnable === true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: "Скасувати"
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
    }
}
