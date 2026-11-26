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

        readonly property string defaultSources: JSON.stringify([
            { id: "", title: "All", installed: 4, updates: 1, failed: false },
            { id: "npm", title: "Node", installed: 2, updates: 1, failed: false },
            { id: "go", title: "Go", installed: 1, updates: 0, failed: false },
            { id: "manual", title: "Вручну", installed: 1, updates: 0, failed: false }
        ])
        property string sourcesJson: defaultSources
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
        property bool deleteRunnable: true

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
        function planDelete(selected) {
            record("planDelete:" + selected)
            return JSON.stringify({
                runnable: deleteRunnable,
                title: deleteRunnable ? "Видалити вибрані пакунки?" : "Нічого видаляти...",
                lines: deleteRunnable ? ["- sudo dnf remove codex"] : []
            })
        }
        function deleteSelected(selected) {
            record("deleteSelected:" + selected)
            busy = true
            upgrading = true
            log = "- sudo dnf remove codex\n"
            // Імітуємо завершення фонової роботи й оновлення таблиці.
            Qt.callLater(function () {
                rowsJson = JSON.stringify([
                    { source: "npm", sourceTitle: "Node", key: "cline", name: "cline", current: "3", latest: "", state: "current", info: "", removable: true },
                    { source: "go", sourceTitle: "Go", key: "air", name: "air", current: "v1", latest: "", state: "unchecked", info: "", removable: false },
                    { source: "manual", sourceTitle: "Вручну", key: "zed", name: "zed", current: "-", latest: "", state: "unknown", info: "", removable: false }
                ])
                busy = false
                upgrading = false
            })
        }

        function reset() {
            sourcesJson = defaultSources
            status = "Готово"
            rowsJson = JSON.stringify([
                { source: "npm", sourceTitle: "Node", key: "codex", name: "codex", current: "1", latest: "2", state: "upgradable", info: "", removable: true },
                { source: "npm", sourceTitle: "Node", key: "cline", name: "cline", current: "3", latest: "", state: "current", info: "", removable: true },
                { source: "go", sourceTitle: "Go", key: "air", name: "air", current: "v1", latest: "", state: "unchecked", info: "", removable: false },
                { source: "manual", sourceTitle: "Вручну", key: "zed", name: "zed", current: "-", latest: "", state: "unknown", info: "", removable: false }
            ])
            currentSource = ""
            onlyUpgradable = false
            search = ""
            busy = false
            upgrading = false
            runnable = true
            deleteRunnable = true
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

        function del() { mouseClick(child("deleteSelectedButton")) }

        function test_no_delete_button_on_rows() {
            verify(!findChild(rowItem(0), "rowDelete"))
            verify(!findChild(rowItem(2), "rowDelete"))
            verify(!findChild(rowItem(3), "rowDelete"))
        }

        function test_selected_row_has_accent_background() {
            const item = rowItem(0)
            verify(!item.highlighted, "рядок не виділений початково")
            check(0)
            verify(item.highlighted, "вибраний рядок має бути підсвічений")
            check(0)
            verify(!item.highlighted, "зняття виділення повертає звичайний вигляд")
        }

        function test_bottom_delete_button_follows_removable_selection() {
            const button = child("deleteSelectedButton")
            verify(button)
            compare(button.text, "Видалити вибрані")
            verify(!button.enabled)
            check(2)
            compare(button.text, "Видалити вибрані", "go не видаляється")
            verify(!button.enabled)
            check(0)
            compare(button.text, "Видалити вибрані (1)")
            verify(button.enabled)
        }

        function test_bottom_delete_button_uses_delete_flow() {
            check(0)
            mouseClick(child("deleteSelectedButton"))
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            compare(fake.calls, ['planDelete:[["npm","codex"]]'])
            dialog.accept()
            compare(fake.calls, [
                'planDelete:[["npm","codex"]]',
                'deleteSelected:[["npm","codex"]]'
            ])
        }

        function test_busy_disables_bottom_delete() {
            check(0)
            fake.busy = true
            verify(!child("deleteSelectedButton").enabled)
        }

        function test_delete_needs_confirmation() {
            check(0)
            check(1)
            del(0)
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            compare(fake.calls, ['planDelete:[["npm","codex"],["npm","cline"]]'])
            compare(dialog.title, "Видалити вибрані пакунки?")
            compare(child("confirmOk").text, "Видалити")
            const lines = dialog.contentItem
            lines.forceLayout()
            compare(lines.count, 1)
            compare(lines.itemAtIndex(0).text, "- sudo dnf remove codex")
        }

        function test_delete_confirm_calls_delete_and_clears_selection() {
            check(0)
            del(0)
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            dialog.accept()
            compare(fake.calls, [
                'planDelete:[["npm","codex"]]',
                'deleteSelected:[["npm","codex"]]'
            ])
            compare(child("upgradeButton").text, "Upgrade усе")
        }

        function test_delete_cancel_keeps_selection() {
            check(0)
            del(0)
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            dialog.reject()
            compare(fake.calls, ['planDelete:[["npm","codex"]]'])
            compare(child("upgradeButton").text, "Upgrade вибрані (1)")
        }

        function test_delete_empty_plan_disables_confirm() {
            fake.deleteRunnable = false
            check(0)
            del(0)
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            verify(!child("confirmOk").enabled)
            compare(dialog.title, "Нічого видаляти...")
        }

        function test_delete_refreshes_rows_and_opens_log() {
            const panel = child("logPanel")
            verify(!panel.visible)
            check(0)
            del(0)
            const dialog = child("confirmDialog")
            tryVerify(() => dialog.opened)
            dialog.accept()
            // Фейковий бекенд спочатку зайнятий, потім оновлює рядки.
            verify(fake.busy)
            verify(panel.visible)
            tryVerify(() => child("rowList").count === 3)
            verify(!fake.busy)
            compare(fake.log, "- sudo dnf remove codex\n")
        }

        // Бекенд віддає лише джерела цього комп'ютера — вікно показує саме їх.
        function test_sidebar_shows_only_the_hosts_sources() {
            fake.sourcesJson = JSON.stringify([
                { id: "", title: "All", installed: 3, updates: 0, failed: false },
                { id: "apt", title: "APT", installed: 2, updates: 0, failed: false },
                { id: "snap", title: "Snap", installed: 1, updates: 0, failed: false }
            ])
            const list = child("sourceList")
            tryCompare(list, "count", 3)
            list.forceLayout()
            const titles = []
            for (let i = 0; i < list.count; i++)
                titles.push(list.itemAtIndex(i).modelData.title)
            compare(titles, ["All", "APT", "Snap"])
            mouseClick(list.itemAtIndex(1))
            compare(fake.calls, ["source:apt"])
            compare(child("upgradeButton").text, "Upgrade усе: APT")
        }

        function test_status_names_skipped_sources() {
            fake.status = "Оновлення: пропущено Pacman. Деталі в журналі"
            compare(child("statusLabel").text, "Оновлення: пропущено Pacman. Деталі в журналі")
        }
    }
}
