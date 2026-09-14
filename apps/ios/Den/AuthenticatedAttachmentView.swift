import SwiftUI
import AVKit
import UniformTypeIdentifiers
import ImageIO
import DenAPI

/// A custom-scheme AVAsset hands every byte request back to Den's authenticated,
/// no-redirect session. AVPlayer never receives a bearer token or a network URL.
@MainActor final class ProtectedPlayback: NSObject, @preconcurrency AVAssetResourceLoaderDelegate {
    let player: AVPlayer
    private let asset: AVURLAsset
    private let service: DenService
    private let upload: API.Upload
    private var tasks: [ObjectIdentifier: Task<Void, Never>] = [:]
    private var statusObservation: NSKeyValueObservation?
    private let onError: @MainActor (String) -> Void

    init(upload: API.Upload, service: DenService, onError: @escaping @MainActor (String) -> Void) {
        self.upload = upload; self.service = service; self.onError = onError
        let url = URL(string: "den-media://attachment/\(UUID().uuidString)")!
        asset = AVURLAsset(url: url)
        player = AVPlayer()
        super.init()
        asset.resourceLoader.setDelegate(self, queue: .main)
        let item = AVPlayerItem(asset: asset)
        player.replaceCurrentItem(with: item)
        statusObservation = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            guard item.status == .failed else { return }
            Task { @MainActor [weak self] in
                self?.onError("This media could not play. Try again or save it to open in another app.")
            }
        }
    }
    func stop() {
        statusObservation?.invalidate(); statusObservation = nil
        player.pause(); player.replaceCurrentItem(with: nil)
        for task in tasks.values { task.cancel() }
        tasks.removeAll()
        asset.resourceLoader.setDelegate(nil, queue: nil)
    }
    func resourceLoader(_ resourceLoader: AVAssetResourceLoader,
                        shouldWaitForLoadingOfRequestedResource request: AVAssetResourceLoadingRequest) -> Bool {
        guard request.request.url == asset.url else { return false }
        let key = ObjectIdentifier(request)
        tasks[key] = Task { @MainActor [weak self] in
            guard let self else { return }
            defer { tasks[key] = nil }
            do {
                let type = UTType(mimeType: upload.contentType)?.identifier ?? UTType.data.identifier
                if let information = request.contentInformationRequest {
                    if let allowed = information.allowedContentTypes, !allowed.isEmpty, !allowed.contains(type) {
                        throw DenFailure.server(415, "This media format cannot play on this device. Save the file to open it in another app.")
                    }
                    information.contentType = type
                    information.contentLength = upload.size
                    information.isByteRangeAccessSupported = true
                }
                if let data = request.dataRequest {
                    let start = max(data.requestedOffset, data.currentOffset)
                    let (requestedEnd, overflow) = data.requestedOffset.addingReportingOverflow(Int64(data.requestedLength))
                    let end = data.requestsAllDataToEndOfResource || overflow ? upload.size : min(upload.size, requestedEnd)
                    guard start >= 0, end >= start, start <= upload.size else { throw DenFailure.invalidResponse }
                    // Bounded requests also make seeking cancel promptly even for a 1 GB clip.
                    var offset = start
                    while offset < end {
                        try Task.checkCancellation()
                        let upper = min(end, offset + 1_048_576) - 1
                        let bytes = try await ProtectedMedia.range(upload: upload, service: service, start: offset, end: upper)
                        try Task.checkCancellation()
                        guard !request.isCancelled else { throw CancellationError() }
                        data.respond(with: bytes)
                        offset += Int64(bytes.count)
                    }
                }
                try Task.checkCancellation()
                if !request.isCancelled { request.finishLoading() }
            } catch {
                if !request.isCancelled {
                    request.finishLoading(with: error)
                    if !(error is CancellationError) { onError(DenFailure.present(error)) }
                }
            }
        }
        return true
    }
    func resourceLoader(_ resourceLoader: AVAssetResourceLoader, didCancel request: AVAssetResourceLoadingRequest) {
        tasks.removeValue(forKey: ObjectIdentifier(request))?.cancel()
    }
}

enum ProtectedMedia {
    static func filePath(_ upload: API.Upload) -> String { "/uploads/\(upload.id)/file" }
    static func validate(_ response: URLResponse, request: URLRequest, status: Int, type: String? = nil) throws -> HTTPURLResponse {
        guard let http = response as? HTTPURLResponse, http.url == request.url else { throw DenFailure.invalidResponse }
        guard http.statusCode == status else {
            throw DenFailure.server(http.statusCode, http.statusCode == 401 ? "Your session expired. Log in again." : "This attachment could not load. Try again.")
        }
        if let type, http.mimeType?.lowercased() != type.lowercased() { throw DenFailure.invalidResponse }
        return http
    }
    static func range(upload: API.Upload, service: DenService, start: Int64, end: Int64) async throws -> Data {
        guard start >= 0, end >= start, end < upload.size, end - start < 1_048_576 else { throw DenFailure.invalidResponse }
        let request = service.authenticatedRequest(path: filePath(upload), range: "bytes=\(start)-\(end)")
        let (bytes, response) = try await service.session.bytes(for: request)
        defer { bytes.task.cancel() }
        let http = try validate(response, request: request, status: 206, type: upload.contentType)
        let count = end - start + 1
        guard http.value(forHTTPHeaderField: "Content-Range") == "bytes \(start)-\(end)/\(upload.size)",
              http.expectedContentLength == count else { throw DenFailure.invalidResponse }
        var result = Data(); result.reserveCapacity(Int(count))
        for try await byte in bytes {
            guard result.count < count else { throw DenFailure.invalidResponse }
            result.append(byte)
        }
        guard result.count == count else { throw DenFailure.invalidResponse }
        return result
    }
    static func image(upload: API.Upload, service: DenService) async throws -> Data? {
        let thumbnail = upload.thumbnailUrl != nil
        guard thumbnail || upload.size <= 8 * 1_048_576 else { return nil }
        // Never follow a server-provided URL with credentials. Only known attachment routes are used.
        let path = thumbnail ? "/uploads/\(upload.id)/thumbnail" : filePath(upload)
        let request = service.authenticatedRequest(path: path)
        let (bytes, response) = try await service.session.bytes(for: request)
        defer { bytes.task.cancel() }
        let http = try validate(response, request: request, status: 200, type: thumbnail ? "image/png" : upload.contentType)
        let limit = thumbnail ? 2 * 1_048_576 : 8 * 1_048_576
        guard http.expectedContentLength <= limit else { throw DenFailure.invalidResponse }
        var result = Data()
        for try await byte in bytes {
            guard result.count < limit else { throw DenFailure.invalidResponse }
            result.append(byte)
        }
        if http.expectedContentLength >= 0, result.count != http.expectedContentLength { throw DenFailure.invalidResponse }
        return result
    }
    static func download(upload: API.Upload, service: DenService) async throws -> URL {
        let request = service.authenticatedRequest(path: filePath(upload))
        let (temporary, response) = try await service.session.download(for: request)
        defer { try? FileManager.default.removeItem(at: temporary) }
        let http = try validate(response, request: request, status: 200, type: upload.contentType)
        guard http.expectedContentLength == upload.size, try AttachmentFiles.size(temporary) == upload.size else {
            throw DenFailure.invalidResponse
        }
        try Task.checkCancellation()
        let directory = try AttachmentFiles.directory()
        let filename = (upload.filename as NSString).lastPathComponent
        let target = directory.appendingPathComponent(filename.isEmpty ? "Attachment" : filename)
        do {
            try FileManager.default.moveItem(at: temporary, to: target)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: target.path)
            return target
        } catch { try? FileManager.default.removeItem(at: directory); throw error }
    }
    static func preview(_ data: Data) -> UIImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1_600
              ] as CFDictionary) else { return nil }
        return UIImage(cgImage: image)
    }
}

struct AuthenticatedAttachmentView: View {
    let upload: API.Upload
    let store: AppStore
    @State private var preview: UIImage?
    @State private var playback: ProtectedPlayback?
    @State private var error: String?
    @State private var loading = false
    @State private var shareFile: SharedAttachment?
    @State private var sharedURL: URL?
    @State private var downloadTask: Task<Void, Never>?
    private var audiovisual: Bool { upload.contentType.hasPrefix("video/") || upload.contentType.hasPrefix("audio/") }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let preview {
                Image(uiImage: preview).resizable().scaledToFit().frame(maxHeight: 300)
                    .clipShape(RoundedRectangle(cornerRadius: 12)).accessibilityLabel(upload.filename)
            }
            if let playback {
                VideoPlayer(player: playback.player)
                    .frame(height: upload.contentType.hasPrefix("audio/") ? 100 : 240)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .accessibilityLabel("Play \(upload.filename)")
            } else if audiovisual {
                Button(action: play) {
                    Label(upload.contentType.hasPrefix("audio/") ? "Play audio" : "Play video", systemImage: "play.circle.fill")
                        .frame(maxWidth: .infinity, minHeight: 80)
                }.buttonStyle(.bordered)
            }
            HStack(spacing: 8) {
                Image(systemName: audiovisual ? "waveform" : "doc")
                VStack(alignment: .leading) {
                    Text(upload.filename).lineLimit(2)
                    Text(ByteCountFormatter.string(fromByteCount: upload.size, countStyle: .file))
                        .font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 4)
                if loading { ProgressView().accessibilityLabel("Downloading attachment") }
                Button(action: share) { Image(systemName: "square.and.arrow.up").frame(minWidth: 44, minHeight: 44) }
                    .disabled(loading).accessibilityLabel("Save or share \(upload.filename)")
            }
            if let error {
                Text(error).font(.caption).foregroundStyle(.red)
                if audiovisual { Button("Retry playback", action: play) }
                if upload.contentType.hasPrefix("image/") {
                    Button("Retry preview") { downloadTask = Task { await loadPreview() } }
                }
            }
        }
        .task(id: upload.id) { await loadPreview() }
        .sheet(item: $shareFile, onDismiss: clearShare) { file in
            AttachmentShareSheet(url: file.url)
        }
        .onDisappear {
            downloadTask?.cancel(); downloadTask = nil
            playback?.stop(); playback = nil; preview = nil
            clearShare()
        }
    }
    private func play() {
        do {
            error = nil
            playback?.stop()
            playback = ProtectedPlayback(upload: upload, service: try store.activeService()) { error = $0 }
            playback?.player.play()
        } catch { self.error = DenFailure.present(error) }
    }
    private func loadPreview() async {
        guard upload.contentType.hasPrefix("image/") else { return }
        do {
            error = nil
            let expected = store.generation
            let data = try await ProtectedMedia.image(upload: upload, service: store.activeService())
            try store.check(expected); try Task.checkCancellation()
            if let data { preview = ProtectedMedia.preview(data) }
        } catch { if !(error is CancellationError) { self.error = DenFailure.present(error) } }
    }
    private func share() {
        downloadTask = Task { @MainActor in
            loading = true; error = nil
            defer { loading = false }
            do {
                let expected = store.generation
                let url = try await ProtectedMedia.download(upload: upload, service: store.activeService())
                do { try store.check(expected); try Task.checkCancellation() }
                catch { AttachmentFiles.remove(url); throw error }
                sharedURL = url; shareFile = .init(url: url)
            } catch { if !(error is CancellationError) { self.error = DenFailure.present(error) } }
        }
    }
    private func clearShare() {
        if let sharedURL { AttachmentFiles.remove(sharedURL) }
        sharedURL = nil; shareFile = nil
    }
}

private struct SharedAttachment: Identifiable {
    let id = UUID()
    let url: URL
}
private struct AttachmentShareSheet: UIViewControllerRepresentable {
    let url: URL
    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [url], applicationActivities: nil)
    }
    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
