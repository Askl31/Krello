import Cocoa
import SQLite3

struct KrelloList {
    let id: String
    let title: String
    let boardId: String
    let boardTitle: String
}

class KeyPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

final class QuickAddApp: NSObject, NSApplicationDelegate, NSTextFieldDelegate, NSWindowDelegate {
    var window: KeyPanel!
    var textField: NSTextField!
    var listPopup: NSPopUpButton!
    var lists: [KrelloList] = []
    let dbPath: String
    var globalMonitor: Any?

    init(dbPath: String) {
        self.dbPath = dbPath
        super.init()
    }

    func loadLists() {
        var db: OpaquePointer?
        if sqlite3_open_v2(dbPath, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK {
            let query = """
            SELECT l.id, l.title, b.id, b.title
            FROM lists l
            JOIN boards b ON l.board_id = b.id
            WHERE l.archived = 0 AND b.archived = 0
            ORDER BY b.title ASC, l.position ASC
            """
            var stmt: OpaquePointer?
            if sqlite3_prepare_v2(db, query, -1, &stmt, nil) == SQLITE_OK {
                while sqlite3_step(stmt) == SQLITE_ROW {
                    let lid = String(cString: sqlite3_column_text(stmt, 0))
                    let ltitle = String(cString: sqlite3_column_text(stmt, 1))
                    let bid = String(cString: sqlite3_column_text(stmt, 2))
                    let btitle = String(cString: sqlite3_column_text(stmt, 3))
                    lists.append(KrelloList(id: lid, title: ltitle, boardId: bid, boardTitle: btitle))
                }
                sqlite3_finalize(stmt)
            }
            sqlite3_close(db)
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        loadLists()
        buildUI()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)

        // Dismiss when user clicks outside the panel (Spotlight-like behavior)
        globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { _ in
            NSApp.terminate(nil)
        }
    }

    func buildUI() {
        let width: CGFloat = 540
        let height: CGFloat = 156
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1200, height: 800)
        let x = screen.origin.x + (screen.width - width) / 2
        let y = screen.origin.y + screen.height * 0.65 - height / 2

        window = KeyPanel(
            contentRect: NSRect(x: x, y: y, width: width, height: height),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        window.isFloatingPanel = true
        window.level = .floating
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = true
        window.delegate = self

        let visualEffect = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: width, height: height))
        visualEffect.material = .hudWindow
        visualEffect.blendingMode = .behindWindow
        visualEffect.state = .active
        visualEffect.wantsLayer = true
        visualEffect.layer?.cornerRadius = 16
        visualEffect.layer?.masksToBounds = true
        visualEffect.layer?.borderColor = NSColor.white.withAlphaComponent(0.18).cgColor
        visualEffect.layer?.borderWidth = 1.0

        // Header: Icon + Title + Hint
        let iconBadge = NSTextField(labelWithString: "⚡️")
        iconBadge.frame = NSRect(x: 20, y: height - 33, width: 22, height: 20)
        iconBadge.font = NSFont.systemFont(ofSize: 14)
        visualEffect.addSubview(iconBadge)

        let header = NSTextField(labelWithString: "Ajouter une tâche à Krello")
        header.frame = NSRect(x: 44, y: height - 34, width: 280, height: 20)
        header.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        header.textColor = NSColor.labelColor
        visualEffect.addSubview(header)

        let hint = NSTextField(labelWithString: "Échap pour fermer")
        hint.frame = NSRect(x: width - 150, y: height - 34, width: 130, height: 20)
        hint.alignment = .right
        hint.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        hint.textColor = NSColor.secondaryLabelColor
        visualEffect.addSubview(hint)

        // Text input field
        textField = NSTextField(frame: NSRect(x: 20, y: height - 80, width: width - 40, height: 38))
        textField.font = NSFont.systemFont(ofSize: 15, weight: .regular)
        textField.placeholderString = "Titre de la tâche… (ex: Préparer le bilan @Inbox)"
        textField.focusRingType = .none
        textField.wantsLayer = true
        textField.layer?.cornerRadius = 8
        textField.layer?.borderWidth = 1.0
        textField.layer?.borderColor = NSColor.separatorColor.cgColor
        textField.delegate = self
        visualEffect.addSubview(textField)

        // Bottom row: List selector & Action buttons
        let listLabel = NSTextField(labelWithString: "Liste :")
        listLabel.frame = NSRect(x: 20, y: 20, width: 45, height: 20)
        listLabel.font = NSFont.systemFont(ofSize: 12)
        listLabel.textColor = NSColor.secondaryLabelColor
        visualEffect.addSubview(listLabel)

        listPopup = NSPopUpButton(frame: NSRect(x: 65, y: 16, width: 260, height: 26), pullsDown: false)
        listPopup.font = NSFont.systemFont(ofSize: 12)
        if lists.isEmpty {
            listPopup.addItem(withTitle: "Aucune liste disponible")
            listPopup.isEnabled = false
        } else {
            for l in lists {
                listPopup.addItem(withTitle: "\(l.title) · \(l.boardTitle)")
            }
        }
        visualEffect.addSubview(listPopup)

        let cancelBtn = NSButton(title: "Annuler", target: self, action: #selector(cancelAction))
        cancelBtn.frame = NSRect(x: width - 185, y: 16, width: 75, height: 26)
        cancelBtn.bezelStyle = .rounded
        cancelBtn.keyEquivalent = "\u{1b}" // Escape
        visualEffect.addSubview(cancelBtn)

        let addBtn = NSButton(title: "Ajouter", target: self, action: #selector(submitAction))
        addBtn.frame = NSRect(x: width - 100, y: 16, width: 80, height: 26)
        addBtn.bezelStyle = .rounded
        addBtn.keyEquivalent = "\r" // Enter
        visualEffect.addSubview(addBtn)

        window.contentView = visualEffect
        window.makeFirstResponder(textField)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            submitAction()
            return true
        } else if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            cancelAction()
            return true
        }
        return false
    }

    func controlTextDidChange(_ obj: Notification) {
        let text = textField.stringValue
        if let atRange = text.range(of: "@", options: .backwards) {
            let afterAt = String(text[atRange.upperBound...]).trimmingCharacters(in: .whitespaces)
            if !afterAt.isEmpty {
                for (idx, l) in lists.enumerated() {
                    if l.title.localizedCaseInsensitiveContains(afterAt) {
                        listPopup.selectItem(at: idx)
                        break
                    }
                }
            }
        }
    }

    @objc func cancelAction() {
        NSApp.terminate(nil)
    }

    @objc func submitAction() {
        var text = textField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty {
            NSSound.beep()
            return
        }

        var targetList: KrelloList? = nil
        if let atRange = text.range(of: "@", options: .backwards) {
            let tag = String(text[atRange.upperBound...]).trimmingCharacters(in: .whitespaces)
            for l in lists {
                if l.title.localizedCaseInsensitiveCompare(tag) == .orderedSame || l.title.localizedCaseInsensitiveContains(tag) {
                    targetList = l
                    text = String(text[..<atRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
                    break
                }
            }
        }

        if targetList == nil && !lists.isEmpty {
            let sel = max(0, listPopup.indexOfSelectedItem)
            if sel < lists.count {
                targetList = lists[sel]
            }
        }

        let finalTitle = text.isEmpty ? "Nouvelle tâche" : text
        let listTitle = targetList?.title ?? "Inbox"

        // Envoi au serveur HTTP Krello s'il tourne (8890 puis 8787)
        var sentHttp = false
        for port in [8890, 8787] {
            if let encodedTitle = finalTitle.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
               let encodedList = listTitle.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
               let url = URL(string: "http://127.0.0.1:\(port)/api/v1/deep-link") {
                var req = URLRequest(url: url)
                req.httpMethod = "POST"
                req.setValue("application/json", forHTTPHeaderField: "Content-Type")
                req.timeoutInterval = 0.5
                let body = ["url": "krello://task/new?title=\(encodedTitle)&list=\(encodedList)"]
                req.httpBody = try? JSONSerialization.data(withJSONObject: body)

                let sem = DispatchSemaphore(value: 0)
                var success = false
                let task = URLSession.shared.dataTask(with: req) { _, resp, _ in
                    if let httpResp = resp as? HTTPURLResponse, httpResp.statusCode == 200 {
                        success = true
                    }
                    sem.signal()
                }
                task.resume()
                _ = sem.wait(timeout: .now() + 0.6)
                if success {
                    sentHttp = true
                    break
                }
            }
        }

        // Repli SQLite direct si Krello est fermé
        if !sentHttp, let target = targetList {
            var db: OpaquePointer?
            if sqlite3_open(dbPath, &db) == SQLITE_OK {
                let cardId = "card_" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
                let isoFormatter = ISO8601DateFormatter()
                let now = isoFormatter.string(from: Date())
                let insertSql = """
                INSERT INTO cards (id, board_id, list_id, title, description, position, created_at, updated_at)
                VALUES (?, ?, ?, ?, '', (SELECT COALESCE(MAX(position), 0) + 1.0 FROM cards WHERE list_id = ?), ?, ?)
                """
                var stmt: OpaquePointer?
                if sqlite3_prepare_v2(db, insertSql, -1, &stmt, nil) == SQLITE_OK {
                    sqlite3_bind_text(stmt, 1, (cardId as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 2, (target.boardId as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 3, (target.id as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 4, (finalTitle as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 5, (target.id as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 6, (now as NSString).utf8String, -1, nil)
                    sqlite3_bind_text(stmt, 7, (now as NSString).utf8String, -1, nil)
                    sqlite3_step(stmt)
                    sqlite3_finalize(stmt)
                }
                sqlite3_close(db)
            }
        }

        // Notification native macOS
        let msg = "« \(finalTitle) » ajoutée dans \(listTitle)"
        let script = "display notification \"\(msg.replacingOccurrences(of: "\"", with: "\\\""))\" with title \"Krello\""
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        proc.arguments = ["-e", script]
        try? proc.run()

        NSApp.terminate(nil)
    }
}

let db = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : (NSHomeDirectory() + "/Library/Application Support/com.krello.app/krello.db")
let app = NSApplication.shared
let delegate = QuickAddApp(dbPath: db)
app.delegate = delegate
app.run()
