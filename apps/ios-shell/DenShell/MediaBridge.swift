import OSLog
// The SDK declares the picker's `Mode.compact` as a plain static var, which Swift 6 refuses without this.
@preconcurrency import PhotosUI
import UIKit
import UniformTypeIdentifiers
import WebKit

// Photos and sharing, next to `call`, `push` and `haptic` on `window.denShell`.
// `photos()` opens iOS's photo picker in one step (the page's own <input> puts WebKit's "Photo Library / Take Photo /
// Choose File" menu first). It needs no access to the library: the picker runs outside the app and hands over only
// what was picked. `share({url, text})` brings up iOS's share sheet, which `navigator.share` didn't in Den Shell.
//
// How a picked file reaches the page: the shell keeps it as a temporary file, and the page reads it back a piece at a
// time (`photoChunk`), base64 over the same reply channel the call bridge uses, and makes a File of the pieces. A
// custom URL scheme the page fetches would be simpler, but WebKit refuses one from an https page (mixed content), and
// Den is always https. Pieces keep every message small whatever the video's size. Each temporary file goes once its
// last piece is read, or at once when it's over the upload limit; any left over (a page that reloaded half way) go at
// the next launch.
//
// The strip of recent photos at the top of the + sheet is the same picker, embedded as one row (iOS 17's compact
// mode). It sits over the web view, where the page says its slot is, and only while the sheet is still: the page hides
// it while anything moves it (shell-media.ts says why). A tap ticks a photo and hands it over as a pick would; another
// tap unticks it and the page takes it out of the tray.

private let log = Logger(subsystem: "app.denchat.shell", category: "media")

/// One message from the page, checked before anything acts on it.
enum MediaCommand: Equatable {
    case pick
    case read(id: String, offset: Int)
    /// `from`: what was tapped, in the web view's points, for the iPad's popover.
    case share(url: URL, text: String?, from: CGRect?)
    /// The strip, in one opening of the + sheet (`session`). `rect`: its slot in the web view's points.
    /// `selected`: photos to show ticked, by the library's ids.
    case stripShow(session: Int, rect: CGRect, radius: Double, selected: [String])
    case stripHide(session: Int)
    case stripClose(session: Int)

    init(_ body: Any) throws {
        guard let body = body as? [String: Any], let type = body["type"] as? String else { throw CallError(message: "Unreadable media message.") }
        func number(_ value: Any?) -> Double? {
            guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.doubleValue.isFinite else { return nil }
            return value.doubleValue
        }
        switch type {
        case "pick": self = .pick
        case "read":
            guard let id = body["id"] as? String, !id.isEmpty, let offset = number(body["offset"]), offset >= 0,
                  offset == offset.rounded(), offset < 1e15 else { throw CallError(message: "Missing id or offset.") }
            self = .read(id: id, offset: Int(offset))
        case "share":
            guard let link = body["url"] as? String, let url = URL(string: link), ["http", "https"].contains(url.scheme?.lowercased()),
                  url.host()?.isEmpty == false else { throw CallError(message: "Only web links can be shared.") }
            let text = body["text"] as? String
            guard text != nil || body["text"] == nil || body["text"] is NSNull else { throw CallError(message: "Unreadable text.") }
            var from: CGRect?
            if let rect = body["from"] as? [String: Any], let x = number(rect["x"]), let y = number(rect["y"]),
               let width = number(rect["width"]), let height = number(rect["height"]), width >= 0, height >= 0 {
                from = CGRect(x: x, y: y, width: width, height: height)
            }
            self = .share(url: url, text: text?.isEmpty == false ? String(text!.prefix(1000)) : nil, from: from)
        case "stripShow", "stripHide", "stripClose":
            guard let session = number(body["session"]), session >= 0, session == session.rounded(), session < 1e15 else {
                throw CallError(message: "Missing session.")
            }
            let id = Int(session)
            if type == "stripHide" { self = .stripHide(session: id); return }
            if type == "stripClose" { self = .stripClose(session: id); return }
            guard let rect = body["rect"] as? [String: Any], let x = number(rect["x"]), let y = number(rect["y"]),
                  let width = number(rect["width"]), let height = number(rect["height"]), width > 0, height > 0,
                  max(abs(x), abs(y), width, height) < 1e5, let radius = number(body["radius"]), radius >= 0,
                  let selected = body["selected"] as? [String] else { throw CallError(message: "Unreadable strip.") }
            self = .stripShow(session: id, rect: CGRect(x: x, y: y, width: width, height: height), radius: min(radius, height / 2),
                              selected: Array(selected.prefix(MediaBridge.limit)))
        default: throw CallError(message: "Unknown media message.")
        }
    }
}

/// The picker's answer, handed from its delegate call to the waiting `pick` on the main actor.
struct Picked: @unchecked Sendable {
    let results: [PHPickerResult]
}

/// What a pick hands the page: the files it can read, and how many picked items couldn't be kept.
struct Pick: Encodable {
    let files: [PickedFile]
    let failed: Int
}

/// What the page learns about a picked file before it reads it. One over `maxSize` comes with nothing to read.
struct PickedFile: Encodable, Equatable {
    let id: String
    let name: String
    let type: String
    let size: Int
}

/// What the strip tells the page (shell-media.ts `ShellStripEvent`).
struct StripEvent: Encodable, Equatable {
    let type: String
    var session: Int?
    var asset: String?
    var file: PickedFile?
}

/// The strip in one opening of the + sheet. `ticked`: what's ticked now, each tick with its own id, so a photo
/// unticked and ticked again while it came in arrives once.
@MainActor final class Strip {
    let session: Int
    let picker: PHPickerViewController
    var ticked: [String: UUID]

    init(session: Int, picker: PHPickerViewController, ticked: [String]) {
        self.session = session
        self.picker = picker
        self.ticked = Dictionary(ticked.map { ($0, UUID()) }, uniquingKeysWith: { a, _ in a })
    }
}

@MainActor final class MediaBridge: NSObject, WKScriptMessageHandlerWithReply, PHPickerViewControllerDelegate {
    /// Bytes per piece: about 2.7 MB of base64 per message.
    nonisolated static let pieceSize = 2 << 20
    /// As many as a message sensibly carries; the server takes any number.
    nonisolated static let limit = 10
    /// Den's upload limit, as the page applies it (uploads.svelte.ts). A bigger file isn't kept.
    static let maxSize = 1 << 30
    let den: Origin
    weak var webView: WKWebView?
    private let folder: URL
    private var picked: [String: URL] = [:]
    private var picking: CheckedContinuation<Picked, Never>?
    /// The picker `picking` waits for.
    private weak var shown: PHPickerViewController?
    private var strip: Strip?
    /// Photos still coming in, by strip, so closing one can wait for them.
    private var stripLoads: [Int: [Task<Void, Never>]] = [:]
    /// Counts pages: photos coming in for one that has gone aren't wanted.
    private var page = 0
    private var keyboardMoving = false

    init(den: Origin) {
        self.den = den
        folder = FileManager.default.temporaryDirectory.appending(path: "picked", directoryHint: .isDirectory)
        try? FileManager.default.removeItem(at: folder)
        super.init()
        // The keyboard moves the web view's bottom edge and the sheet with it: the strip steps aside until it's done.
        for (name, moving) in [(UIResponder.keyboardWillChangeFrameNotification, true), (UIResponder.keyboardDidChangeFrameNotification, false)] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.keyboard(moving: moving) }
            }
        }
    }

    /// Adds `photos`, `photoChunk` and `share` to the `window.denShell` that CallBridge's script made, so install after it.
    func install(in controller: WKUserContentController) {
        // A JSON string literal is a valid JavaScript string literal.
        let origin = String(data: try! JSONEncoder().encode(den.serialized), encoding: .utf8)!
        let source = """
        if (location.origin === \(origin) && window.denShell) {
          const post = m => window.webkit.messageHandlers.denMedia.postMessage(m)
          Object.assign(window.denShell, {
            photos: () => post({ type: 'pick' }).then(r => JSON.parse(r)),
            photoChunk: (id, offset) => post({ type: 'read', id, offset }),
            share: s => post({ type: 'share', url: s.url, text: s.text, from: s.from }).then(() => {}),
            photoStrip: c => post(c).then(() => {}),
            onPhotoStrip: null,
          })
        }
        """
        controller.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        controller.addScriptMessageHandler(self, contentWorld: .page, name: "denMedia")
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) async -> (Any?, String?) {
        guard CallBridge.allows(isMainFrame: message.frameInfo.isMainFrame, origin: Origin(message.frameInfo.securityOrigin), den: den) else {
            log.error("media message refused from \(message.frameInfo.securityOrigin.host, privacy: .public)")
            return (nil, "Not allowed.")
        }
        do {
            switch try MediaCommand(message.body) {
            case .pick: return (String(data: try JSONEncoder().encode(try await pick()), encoding: .utf8), nil)
            case .read(let id, let offset): return (try await read(id, offset: offset), nil)
            case .share(let url, let text, let from): try share(url, text: text, from: from); return (nil, nil)
            case .stripShow(let session, let rect, let radius, let selected): try showStrip(session, rect: rect, radius: radius, selected: selected)
            case .stripHide(let session): if strip?.session == session { strip?.picker.view.isHidden = true }
            case .stripClose(let session): await closeStrip(session)
            }
            return (nil, nil)
        } catch {
            return (nil, error.localizedDescription)
        }
    }

    // MARK: Photos

    private func pick() async throws -> Pick {
        if picking != nil {
            if shown?.viewIfLoaded?.window != nil { throw CallError(message: "Your photos are already open.") }
            // A picker that went without saying so mustn't block the next one.
            finish([])
        }
        guard let top = presenter() else { throw CallError(message: "Den can't show your photos right now.") }
        var config = PHPickerConfiguration()
        config.filter = .any(of: [.images, .videos])
        config.selectionLimit = Self.limit
        config.selection = .ordered
        // JPEG and H.264 rather than HEIC and HEVC, as Safari's own file picker hands over: what every browser can show.
        config.preferredAssetRepresentationMode = .compatible
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = self
        shown = picker
        let results = await withCheckedContinuation { picking = $0; top.present(picker, animated: true) }.results
        var files: [PickedFile] = []
        for result in results {
            do { files.append(try await keep(result.itemProvider)) } catch { log.error("picked item not kept: \(error, privacy: .public)") }
        }
        log.info("picked \(files.count, privacy: .public) of \(results.count, privacy: .public)")
        return Pick(files: files, failed: results.count - files.count)
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        if let strip, picker === strip.picker { return ticksChanged(strip, results) }
        // A strip's picker that has gone already; `dismiss` would reach past it.
        guard picker.presentingViewController != nil else { return }
        picker.dismiss(animated: true)
        // Only the picker that's being waited for answers.
        if picker === shown { finish(results) }
    }

    private func finish(_ results: [PHPickerResult]) {
        picking?.resume(returning: Picked(results: results))
        picking = nil
        shown = nil
    }

    /// Moves the picked photo or video into the shell's own temporary folder, where the page can read it.
    private func keep(_ provider: NSItemProvider) async throws -> PickedFile {
        let types = provider.registeredTypeIdentifiers.compactMap { UTType($0) }
        guard var type = types.first(where: { $0.conforms(to: .movie) }) ?? types.first(where: { $0.conforms(to: .image) }) else {
            throw CallError(message: "Not a photo or video.")
        }
        if type.conforms(to: .heic) || type.conforms(to: .heif), types.contains(.jpeg) { type = .jpeg }
        let id = UUID().uuidString, folder = folder, chosen = type
        let (url, ext): (URL, String) = try await withCheckedThrowingContinuation { done in
            _ = provider.loadFileRepresentation(forTypeIdentifier: chosen.identifier) { file, error in
                // iOS deletes `file` when this returns: move it out first.
                do {
                    guard let file else { throw error ?? CallError(message: "No file.") }
                    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                    let to = folder.appending(path: id)
                    try FileManager.default.moveItem(at: file, to: to)
                    done.resume(returning: (to, file.pathExtension.isEmpty ? chosen.preferredFilenameExtension ?? "" : file.pathExtension))
                } catch {
                    done.resume(throwing: error)
                }
            }
        }
        let size = (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? 0
        let base = (provider.suggestedName ?? "").replacingOccurrences(of: "/", with: "-")
        let name = (base.isEmpty ? "Photo" : base) + (ext.isEmpty ? "" : ".\(ext)")
        let mime = UTType(filenameExtension: ext)?.preferredMIMEType ?? chosen.preferredMIMEType ?? "application/octet-stream"
        if size > Self.maxSize { try? FileManager.default.removeItem(at: url) } else { picked[id] = url }
        return PickedFile(id: id, name: name, type: mime, size: size)
    }

    private func forget(_ id: String) {
        if let url = picked.removeValue(forKey: id) { try? FileManager.default.removeItem(at: url) }
    }

    private func read(_ id: String, offset: Int) async throws -> String {
        guard let url = picked[id] else { throw CallError(message: "That file is gone.") }
        let (piece, last) = try await Task.detached { try Self.piece(of: url, at: offset) }.value
        if last {
            picked[id] = nil
            try? FileManager.default.removeItem(at: url)
        }
        return piece
    }

    /// Up to `pieceSize` bytes from `offset`, in base64, and whether they reach the end of the file.
    nonisolated static func piece(of url: URL, at offset: Int) throws -> (String, last: Bool) {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        let size = try handle.seekToEnd()
        guard UInt64(offset) <= size else { throw CallError(message: "Past the end of the file.") }
        try handle.seek(toOffset: UInt64(offset))
        let data = try handle.read(upToCount: pieceSize) ?? Data()
        return (data.base64EncodedString(), UInt64(offset) + UInt64(data.count) >= size)
    }

    // MARK: Strip

    private func showStrip(_ session: Int, rect: CGRect, radius: Double, selected: [String]) throws {
        guard let webView, let host = webView.superview, let parent = webView.window?.rootViewController else {
            throw CallError(message: "Den can't show your photos right now.")
        }
        if strip?.session != session {
            removeStrip()
            // A library to say which photo is which (the ids), not to read it: the picker still needs no access.
            var config = PHPickerConfiguration(photoLibrary: .shared())
            config.filter = .any(of: [.images, .videos])
            config.selectionLimit = Self.limit
            config.selection = .continuous
            config.mode = .compact
            config.preferredAssetRepresentationMode = .compatible
            config.preselectedAssetIdentifiers = selected
            config.edgesWithoutContentMargins = .all
            config.disabledCapabilities = [.search, .collectionNavigation, .selectionActions, .stagingArea]
            let picker = PHPickerViewController(configuration: config)
            picker.delegate = self
            picker.view.clipsToBounds = true
            parent.addChild(picker)
            host.addSubview(picker.view)
            picker.didMove(toParent: parent)
            strip = Strip(session: session, picker: picker, ticked: selected)
            log.info("strip opened")
        }
        guard let strip else { return }
        // Light or dark, as the page is.
        var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        if webView.themeColor?.getRed(&r, green: &g, blue: &b, alpha: &a) == true {
            strip.picker.overrideUserInterfaceStyle = 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5 ? .dark : .light
        }
        strip.picker.view.frame = webView.convert(rect, to: host)
        strip.picker.view.layer.cornerRadius = radius
        strip.picker.view.isHidden = keyboardMoving
    }

    /// Off the screen at once. Photos ticked in it still come in.
    private func removeStrip() {
        guard let strip else { return }
        self.strip = nil
        strip.picker.willMove(toParent: nil)
        strip.picker.view.removeFromSuperview()
        strip.picker.removeFromParent()
    }

    /// Answers once every photo ticked in that strip has come in, so the page knows where they go until then.
    private func closeStrip(_ session: Int) async {
        if strip?.session == session { removeStrip() }
        for load in stripLoads.removeValue(forKey: session) ?? [] { await load.value }
    }

    /// The page went (a load, a crash): so does the strip, and what was coming in for it.
    func pageGone() {
        removeStrip()
        stripLoads = [:]
        page += 1
    }

    private func ticksChanged(_ strip: Strip, _ results: [PHPickerResult]) {
        let now = results.compactMap(\.assetIdentifier)
        for asset in strip.ticked.keys.filter({ !now.contains($0) }) {
            strip.ticked[asset] = nil
            Task { await send(StripEvent(type: "removed", session: strip.session, asset: asset)) }
        }
        for result in results {
            guard let asset = result.assetIdentifier, strip.ticked[asset] == nil else { continue }
            let tick = UUID(), page = page, provider = result.itemProvider
            strip.ticked[asset] = tick
            let load = Task { [weak self] in
                guard let self else { return }
                do {
                    let file = try await keep(provider)
                    // Unticked while it came in, or the page went: not wanted.
                    guard strip.ticked[asset] == tick, self.page == page else { return forget(file.id) }
                    await send(StripEvent(type: "added", session: strip.session, asset: asset, file: file))
                } catch {
                    log.error("strip photo not kept: \(error, privacy: .public)")
                    if strip.ticked[asset] == tick, self.page == page { await send(StripEvent(type: "failed", session: strip.session, asset: asset)) }
                }
            }
            stripLoads[strip.session, default: []].append(load)
        }
    }

    private func keyboard(moving: Bool) {
        keyboardMoving = moving
        guard let strip else { return }
        // Where the slot is now is the page's to say.
        if moving { strip.picker.view.isHidden = true } else { Task { await send(StripEvent(type: "place")) } }
    }

    private func send(_ event: StripEvent) async {
        guard let webView, let data = try? JSONEncoder().encode(event), let json = String(data: data, encoding: .utf8) else { return }
        do {
            _ = try await webView.callAsyncJavaScript("window.denShell?.onPhotoStrip?.(JSON.parse(s))", arguments: ["s": json], contentWorld: .page)
        } catch {
            log.error("strip event not delivered: \(error, privacy: .public)")
        }
    }

    // MARK: Share

    private func share(_ url: URL, text: String?, from: CGRect?) throws {
        guard let webView, let top = presenter() else { throw CallError(message: "Den can't share right now.") }
        let sheet = UIActivityViewController(activityItems: (text.map { [$0] } ?? []) + [url], applicationActivities: nil)
        // A popover on iPad, pointing at what was tapped; iPhone ignores it.
        if let popover = sheet.popoverPresentationController {
            popover.sourceView = webView
            popover.sourceRect = from ?? CGRect(x: webView.bounds.midX, y: webView.bounds.maxY, width: 0, height: 0)
        }
        top.present(sheet, animated: true)
        log.info("share sheet shown")
    }

    /// The screen on top, which is where a picker or sheet goes.
    private func presenter() -> UIViewController? {
        var top = webView?.window?.rootViewController
        while let presented = top?.presentedViewController, !presented.isBeingDismissed { top = presented }
        return top
    }
}
