// Тести інтерфейсу з підставним бекендом: нічого в системі не запускається.
//   QT_QPA_PLATFORM=offscreen /usr/lib64/qt6/bin/qmltestrunner -input gui/tests
import QtQuick
import QtTest
import "../qml"

Item {
    width: 960
    height: 600

    QtObject {
        id: fake

        property string sourcesJson: JSON.stringify([
            { id: "", title: "All", installed: 4, updates: 1, failed: false },
            { id: "npm", title: "Node", installed: 2, updates: 1, failed: false },
            { id: "go", title: "Go", installed: 1, updates: 0, failed: false },
            { id: "manual", title: "Вручну", installed: 1, updates: 0, failed: false }
        ])
        property string rowsJson: ""
        property string currentSource: ""
        property bool onlyUpgradable: false
        property string search: ""
        property bool busy: false
        property bool upgrading: false
        property string status: "Готово"
        property string log: ""
        property int updateCount: 1

        property var calls: []
        property bool runnable: true

        function record(call) { calls = calls.concat([call]) }
        function selectSource(id) { record("source:" + id); currentSource = id }
        function setFilter(only, text) {
            record("filter:" + only + ":" + text)
            onlyUpgradable = only
            search = text
        }
        function refresh() { record("refresh") }
        function planUpgrade(selected) {
            record("plan:" + selected)
            return JSON.stringify({ runnable: runnable, title: "Оновити?", lines: ["codex 1 → 2"] })
        }
        function upgrade(selected) { record("upgrade:" + selected) }

        function reset() {
            rowsJson = JSON.stringify([
                { source: "npm", sourceTitle: "Node", key: "codex", name: "codex", current: "1", latest: "2", state: "upgradable", info: "" },
                { source: "npm", sourceTitle: "Node", key: "cline", name: "cline", current: "3", latest: "", state: "current", info: "" },
                { source: "go", sourceTitle: "Go", key: "air", name: "air", current: "v1", latest: "", state: "unchecked", info: "" },
                { source: "manual", sourceTitle: "Вручну", key: "zed", name: "zed", current: "-", latest: "", state: "unknown", info: "" }
            ])
            currentSource = ""
            onlyUpgradable = false
            search = ""
            busy = false
            upgrading = false
            runnable = true
            calls = []
        }
    }

    Component {
        id: factory
        MainView { anchors.fill: parent; backend: fake }
    }

    TestCase {
        name: "MainView"
        when: windowShown

        property var view

        function init() {
            fake.reset()
            view = createTemporaryObject(factory, parent)
            verify(view)
            waitForRendering(view)
        }

        function child(name) {
            const found = findChild(view, name)
            verify(found, "немає елемента " + name)
            return found
        }

        function rowItem(index) {
            const list = child("rowList")
            list.forceLayout()
            const item = list.itemAtIndex(index)
            verify(item, "немає рядка " + index)
            return item
        }

        function check(index) { mouseClick(findChild(rowItem(index), "rowCheck")) }

        function test_shows_sources_and_packages() {
            compare(child("sourceList").count, 4)
            compare(child("rowList").count, 4)
        }

        function test_click_on_source_selects_it() {
            const list = child("sourceList")
            list.forceLayout()
            mouseClick(list.itemAtIndex(1))
            compare(fake.calls, ["source:npm"])
        }

        function test_state_texts() {
            compare(findChild(rowItem(0), "rowLatest").text, "2")
            compare(findChild(rowItem(1), "rowLatest").text, "актуальний")
            compare(findChild(rowItem(2), "rowLatest").text, "")
            compare(findChild(rowItem(3), "rowLatest").text, "вручну")
        }

        function test_upgrade_button_follows_selection() {
            const button = child("upgradeButton")
            compare(button.text, "Upgrade усе")
            check(0)
            compare(button.text, "Upgrade вибрані (1)")
            check(1)
            compare(button.text, "Upgrade вибрані (2)")
            check(0)
            compare(button.text, "Upgrade вибрані (1)")
        }

        function test_upgrade_button_names_current_source() {
            fake.currentSource = "npm"
            compare(child("upgradeButton").text, "Upgrade усе: Node")
        }

        function test_upgrade_needs_confirmation() {
            check(0)
            mouseClick(child("upgradeButton"))
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            compare(fake.calls, ['plan:[["npm","codex"]]'])
            dialog.accept()
            compare(fake.calls[1], 'upgrade:[["npm","codex"]]')
            compare(child("upgradeButton").text, "Upgrade усе")
        }

        function test_cancelled_confirmation_does_nothing() {
            mouseClick(child("upgradeButton"))
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            dialog.reject()
            compare(fake.calls, ["plan:[]"])
        }

        function test_empty_plan_cannot_be_confirmed() {
            fake.runnable = false
            mouseClick(child("upgradeButton"))
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            verify(!child("confirmOk").enabled)
        }

        function test_refresh() {
            mouseClick(child("refreshButton"))
            compare(fake.calls, ["refresh"])
        }

        function test_busy_blocks_actions() {
            fake.busy = true
            verify(!child("refreshButton").enabled)
            verify(!child("upgradeButton").enabled)
        }

        function test_filter_checkbox_follows_backend_after_click() {
            const box = child("onlyUpgradable")
            mouseClick(box)
            compare(fake.calls, ["filter:true:"])
            verify(box.checked)
            // Бекенд може змінити фільтр сам (після Refresh чи Upgrade).
            fake.onlyUpgradable = false
            verify(!box.checked)
        }

        function test_search() {
            child("searchField").text = "cod"
            compare(fake.calls, ["filter:false:cod"])
        }

        function test_manual_cannot_be_upgraded() {
            verify(!findChild(rowItem(3), "rowCheck").enabled)
            fake.currentSource = "manual"
            verify(!child("upgradeButton").enabled)
        }

        function test_select_all_visible_skips_manual() {
            const all = child("selectAll")
            mouseClick(all)
            compare(child("upgradeButton").text, "Upgrade вибрані (3)")
            mouseClick(all)
            compare(child("upgradeButton").text, "Upgrade усе")
        }

        function test_changing_source_clears_selection() {
            check(0)
            fake.currentSource = "go"
            compare(child("upgradeButton").text, "Upgrade усе: Go")
        }

        function test_window_cannot_close_during_upgrade() {
            verify(view.canClose)
            fake.upgrading = true
            verify(!view.canClose)
        }

        function test_log_panel_opens_for_upgrade() {
            const panel = child("logPanel")
            verify(!panel.visible)
            fake.log = "+ sudo dnf upgrade\n"
            fake.upgrading = true
            verify(panel.visible)
        }
    }
}
