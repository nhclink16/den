import Foundation
import UniformTypeIdentifiers
import DenAPI

struct PendingUpload: Identifiable {
    let id: UUID
    let channelId: String
    let filename: String
    let localURL: URL
    var progress: Double = 0
    var upload: API.Upload?
    var error: String?
}

/// Only owned copies enter the pending queue. A picker URL may disappear after its callback.
enum AttachmentFiles {
    static func copy(_ source: URL, id: UUID) throws -> URL {
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        let directory = try directory(id: id)
        let target = directory.appendingPathComponent(source.lastPathComponent)
        do {
            try FileManager.default.copyItem(at: source, to: target)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: target.path)
            return target
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }
    static func directory(id: UUID = UUID()) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("DenAttachments", isDirectory: true)
            .appendingPathComponent(id.uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication])
        var excluded = directory
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try excluded.setResourceValues(values)
        return directory
    }
    static func remove(_ url: URL) {
        // Every owned file has a unique directory. Never remove a picker-provided directory.
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("DenAttachments", isDirectory: true).standardizedFileURL.path
        guard url.deletingLastPathComponent().deletingLastPathComponent().standardizedFileURL.path == root else { return }
        try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
    }
    static func size(_ url: URL) throws -> Int64 {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attributes[.type] as? FileAttributeType == .typeRegular,
              let size = attributes[.size] as? NSNumber, size.int64Value > 0 else {
            throw DenFailure.server(400, "Choose a nonempty file.")
        }
        return size.int64Value
    }
    static func chunk(_ url: URL, offset: Int64, length: Int) throws -> Data {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        try file.seek(toOffset: UInt64(offset))
        guard let bytes = try file.read(upToCount: length), bytes.count == length else {
            throw DenFailure.server(400, "The attachment changed. Remove it and choose the file again.")
        }
        return bytes
    }
}

@MainActor extension AppStore {
    func attach(url: URL, channelId: String) async throws {
        _ = try activeService()
        let expected = generation, id = UUID()
        let owned = try await Task.detached { try AttachmentFiles.copy(url, id: id) }.value
        do { try check(expected); try Task.checkCancellation() }
        catch { AttachmentFiles.remove(owned); throw error }
        pendingUploads.append(.init(id: id, channelId: channelId, filename: url.lastPathComponent, localURL: owned))
        try await retryUpload(id: id)
    }

    func retryUpload(id: UUID) async throws {
        guard uploadTasks[id] == nil, pendingUploads.contains(where: { $0.id == id }) else { return }
        let service = try activeService(), expected = generation
        updatePending(id) { $0.error = nil }
        let work = Task { @MainActor in
            do { try await transferUpload(id: id, service: service, expected: expected) }
            catch {
                if generation == expected {
                    updatePending(id) { $0.error = error is CancellationError ? "Upload paused. Tap Retry to continue." : DenFailure.present(error) }
                }
                throw error
            }
        }
        let cancellation = Task<Void, Never> {
            _ = try? await withTaskCancellationHandler(operation: { try await work.value }, onCancel: { work.cancel() })
        }
        uploadTasks[id] = cancellation
        defer { uploadTasks[id] = nil }
        try await withTaskCancellationHandler(operation: { try await work.value }, onCancel: { work.cancel() })
    }

    private func transferUpload(id: UUID, service: DenService, expected: UUID) async throws {
        guard let pending = pendingUploads.first(where: { $0.id == id }) else { throw CancellationError() }
        let size = try await Task.detached { try AttachmentFiles.size(pending.localURL) }.value
        try check(expected); try Task.checkCancellation()
        var upload: API.Upload
        if let previous = pending.upload {
            // A PATCH may have succeeded even when its response was lost. The server is authoritative.
            do { upload = try await service.uploadStatus(id: previous.id) }
            catch {
                guard uploadWasRemoved(error) else { throw error }
                try check(expected); try Task.checkCancellation()
                updatePending(id) { $0.upload = nil; $0.progress = 0 }
                let type = UTType(filenameExtension: pending.localURL.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
                upload = try await service.beginUpload(channelId: pending.channelId, filename: pending.filename, contentType: type, size: size)
            }
        } else {
            let type = UTType(filenameExtension: pending.localURL.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            upload = try await service.beginUpload(channelId: pending.channelId, filename: pending.filename, contentType: type, size: size)
        }
        // Save a newly allocated id before checking cancellation so Remove can still delete it.
        if generation == expected { updatePending(id) { $0.upload = upload } }
        else { try? await service.deleteUpload(id: upload.id); throw CancellationError() }
        try Task.checkCancellation()
        guard upload.channelId == pending.channelId, upload.size == size,
              upload.offset >= 0, upload.offset <= size else { throw DenFailure.invalidResponse }
        updatePending(id) { $0.progress = Double(upload.offset) / Double(size) }
        while upload.offset < size && !upload.complete {
            try Task.checkCancellation()
            let offset = upload.offset, length = Int(min(1_048_576, size - offset))
            let data = try await Task.detached { try AttachmentFiles.chunk(pending.localURL, offset: offset, length: length) }.value
            try check(expected); try Task.checkCancellation()
            let next = try await service.uploadChunk(id: upload.id, offset: offset, data: data)
            try check(expected)
            guard next.id == upload.id, next.channelId == pending.channelId,
                  next.size == size, next.offset == offset + Int64(length) else { throw DenFailure.invalidResponse }
            upload = next
            updatePending(id) { $0.upload = upload; $0.progress = Double(upload.offset) / Double(size) }
        }
        try Task.checkCancellation()
        if !upload.complete {
            upload = try await service.completeUpload(id: upload.id)
            try check(expected)
        }
        guard upload.complete, upload.offset == size else { throw DenFailure.invalidResponse }
        updatePending(id) { $0.upload = upload; $0.progress = 1; $0.error = nil }
    }

    func removeUpload(id: UUID) async throws {
        let service = try activeService(), expected = generation
        if let task = uploadTasks[id] { task.cancel(); await task.value; try check(expected) }
        guard let pending = pendingUploads.first(where: { $0.id == id }) else { return }
        if let upload = pending.upload {
            do { try await service.deleteUpload(id: upload.id); try check(expected) }
            catch {
                if !uploadWasRemoved(error) {
                    updatePending(id) { $0.error = "Could not remove this attachment. Try Remove again." }
                    throw error
                }
                try check(expected)
            }
        }
        discardPending(id: id)
    }

    func cancelPendingUploads() async throws {
        for task in uploadTasks.values { task.cancel() }
        for id in pendingUploads.map(\.id) { try await removeUpload(id: id) }
    }

    /// Called only after the server attaches these uploads to a sent message.
    func discardPending(id: UUID) {
        guard let pending = pendingUploads.first(where: { $0.id == id }) else { return }
        AttachmentFiles.remove(pending.localURL)
        pendingUploads.removeAll { $0.id == id }
    }
    private func uploadWasRemoved(_ error: Error) -> Bool {
        if let client = error as? ClientError { return uploadWasRemoved(client.underlyingError) }
        if let failure = error as? DenFailure, case .server(404, _) = failure { return true }
        return false
    }
    private func updatePending(_ id: UUID, _ change: (inout PendingUpload) -> Void) {
        guard let index = pendingUploads.firstIndex(where: { $0.id == id }) else { return }
        change(&pendingUploads[index])
    }
}
