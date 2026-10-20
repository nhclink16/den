import OSLog
import PhotosUI
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
// last piece is read; any left over (a page that reloaded half way) go at the next launch.

private let log = Logger(subsystem: "app.denchat.shell", category: "media")

/// One message from the page, checked before anything acts on it.
enum MediaCommand: Equatable {
    case pick
    case read(id: String, offset: Int)
    /// `from`: what was tapped, in the web view's points, for the iPad's popover.
    case share(url: URL, text: String?, from: CGRect?)

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
        default: throw CallError(message: "Unknown media message.")
        }
    }
}

/// The picker's answer, handed from its delegate call to the waiting `pick` on the main actor.
struct Picked: @unchecked Sendable {
    let results: [PHPickerResult]
}

/// What the page learns about a picked file before it reads it.
struct PickedFile: Encodable, Equatable {
    let id: String
    let name: String
    let type: String
    let size: Int
}

@MainActor final class MediaBridge: NSObject, WKScriptMessageHandlerWithReply, PHPickerViewControllerDelegate {
    /// Bytes per piece: about 2.7 MB of base64 per message.
    nonisolated static let pieceSize = 2 << 20
    /// As many as a message sensibly carries; the server takes any number.
    static let limit = 10
    let den: Origin
    weak var webView: WKWebView?
    private let folder: URL
    private var picked: [String: URL] = [:]
    private var picking: CheckedContinuation<Picked, Never>?

    init(den: Origin) {
        self.den = den
        folder = FileManager.default.temporaryDirectory.appending(path: "picked", directoryHint: .isDirectory)
        try? FileManager.default.removeItem(at: folder)
        super.init()
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
            }
        } catch {
            return (nil, error.localizedDescription)
        }
    }

    // MARK: Photos

    private func pick() async throws -> [PickedFile] {
        guard let top = presenter() else { throw CallError(message: "Den can't show your photos right now.") }
        // A picker that went without saying so mustn't block the next one.
        finish([])
        var config = PHPickerConfiguration()
        config.filter = .any(of: [.images, .videos])
        config.selectionLimit = Self.limit
        config.selection = .ordered
        // JPEG and H.264 rather than HEIC and HEVC, as Safari's own file picker hands over: what every browser can show.
        config.preferredAssetRepresentationMode = .compatible
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = self
        let results = await withCheckedContinuation { picking = $0; top.present(picker, animated: true) }.results
        var files: [PickedFile] = []
        for result in results {
            do { files.append(try await keep(result.itemProvider)) } catch { log.error("picked item not kept: \(error, privacy: .public)") }
        }
        log.info("picked \(files.count, privacy: .public) of \(results.count, privacy: .public)")
        if files.isEmpty && !results.isEmpty { throw CallError(message: "Those couldn't be added.") }
        return files
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        finish(results)
    }

    private func finish(_ results: [PHPickerResult]) {
        picking?.resume(returning: Picked(results: results))
        picking = nil
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
        picked[id] = url
        return PickedFile(id: id, name: name, type: mime, size: size)
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
