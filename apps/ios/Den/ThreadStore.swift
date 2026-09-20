import DenAPI
import Foundation

@MainActor extension AppStore {
    func threadForRoot(_ rootId: String) -> API.ThreadSummary? {
        threadMetadata.values.first { $0.rootMessageId == rootId }
    }

    func threads(in channelId: String, includeResolved: Bool = false) -> [API.ThreadSummary] {
        threadMetadata.values
            .filter { $0.channelId == channelId && (includeResolved || $0.resolvedAt == nil) }
            .sorted {
                if $0.lastActivityAt == $1.lastActivityAt { return $0.id > $1.id }
                return $0.lastActivityAt > $1.lastActivityAt
            }
    }

    func rootMessage(_ rootId: String) -> API.Message? {
        threadRoots[rootId] ?? messages.values.lazy.flatMap { $0 }.first { $0.id == rootId }
    }

    func cachedMessage(id: String, channelId: String) -> API.Message? {
        if let value = messages[channelId]?.first(where: { $0.id == id }) { return value }
        if let value = threadRoots[id], value.channelId == channelId { return value }
        return threadMessages.values.lazy.flatMap { $0 }
            .first { $0.id == id && $0.channelId == channelId }
    }

    func openReplyReference(from message: API.Message, parentId: String) {
        if let threadId = message.threadId {
            selectThread(channelId: message.channelId,
                         rootId: threadMetadata[threadId]?.rootMessageId,
                         threadId: threadId, messageId: parentId)
        } else {
            selectChannel(message.channelId, messageId: parentId)
        }
    }

    @discardableResult
    func loadThreads(channelId: String, resolved: Bool? = nil, unreadOnly: Bool? = nil,
                     before: String? = nil, limit: Int32 = 50) async throws -> [API.ThreadView] {
        let expected = generation
        let metadataSnapshot = metadataOrder.snapshot()
        let readSnapshot = readOrder.snapshot()
        let views = try await activeService().threads(channelId: channelId, resolved: resolved,
            unreadOnly: unreadOnly, before: before, limit: limit)
        try check(expected)
        for view in views {
            guard view.thread.channelId == channelId, view.readState.channelId == channelId,
                  view.readState.threadId == view.thread.id else { throw DenFailure.invalidResponse }
            applyThreadMetadata(view.thread, snapshot: metadataSnapshot)
            applyThreadRead(view.readState, snapshot: readSnapshot)
        }
        saveCache()
        return views
    }

    func loadAllThreads(channelId: String, resolved: Bool? = nil,
                        unreadOnly: Bool? = nil) async throws -> [API.ThreadView] {
        var result: [API.ThreadView] = []
        var before: String?
        while true {
            let page = try await loadThreads(channelId: channelId, resolved: resolved,
                                             unreadOnly: unreadOnly, before: before, limit: 50)
            result.append(contentsOf: page)
            guard page.count == 50, let cursor = page.last?.thread.id, cursor != before else { return result }
            before = cursor
        }
    }

    @discardableResult
    func loadThread(id: String) async throws -> API.ThreadView {
        let expected = generation
        let metadataToken = metadataOrder.begin(.thread(id))
        let readToken = readOrder.begin(.thread(id))
        let view = try await activeService().thread(id: id)
        try check(expected)
        guard view.thread.id == id, view.readState.threadId == id,
              view.thread.channelId == view.readState.channelId else { throw DenFailure.invalidResponse }
        applyThreadMetadata(view.thread, token: metadataToken)
        applyThreadRead(view.readState, token: readToken)
        if selectedThread?.threadId == id {
            selectedThread?.rootId = view.thread.rootMessageId
        }
        saveCache()
        return view
    }

    @discardableResult
    func loadThreadRoot(id: String) async throws -> API.Message {
        let summary: API.ThreadSummary
        if let known = threadMetadata[id] { summary = known }
        else { summary = try await loadThread(id: id).thread }
        let expected = generation
        let root = try await activeService().message(id: summary.rootMessageId)
        try check(expected)
        guard root.channelId == summary.channelId, root.threadId == nil else { throw DenFailure.invalidResponse }
        threadRoots[root.id] = root
        mergeInlineThread(root)
        saveCache()
        return root
    }

    func loadThreadMessages(id: String, before: String? = nil) async throws {
        let expected = generation
        let values = try await activeService().threadMessages(id: id, before: before)
        try check(expected)
        guard values.allSatisfy({ $0.threadId == id }) else { throw DenFailure.invalidResponse }
        if before == nil {
            threadMessages[id] = values.sorted { $0.id < $1.id }
            threadHasNewer[id] = false
        } else {
            mergeThreadMessages(values, threadId: id)
        }
        saveCache()
    }

    func loadThreadConversation(id: String, target: String?) async throws {
        let expected = generation
        try await loadThreadMessages(id: id)
        let newest = threadMessages[id]?.last?.id
        guard let target,
              !(threadMessages[id] ?? []).contains(where: { $0.id == target }) else { return }
        let service = try activeService()
        let anchor = try await service.message(id: target)
        guard anchor.threadId == id else { throw DenFailure.invalidResponse }
        async let earlier = service.threadMessages(id: id, before: target)
        async let later = service.threadMessages(id: id, after: target)
        let window = try await earlier + [anchor] + later
        try check(expected)
        guard window.allSatisfy({ $0.threadId == id }) else { throw DenFailure.invalidResponse }
        threadMessages[id] = window.sorted { $0.id < $1.id }
        threadHasNewer[id] = newest != nil && threadMessages[id]?.last?.id != newest
        saveCache()
    }

    func prepareThreadReference(threadId: String, parentId: String) async throws -> Bool {
        if threadMetadata[threadId]?.rootMessageId == parentId {
            if rootMessage(parentId) != nil { return true }
            guard !offline else { return false }
            _ = try await loadThreadRoot(id: threadId)
            return rootMessage(parentId) != nil
        }
        if threadMessages[threadId]?.contains(where: { $0.id == parentId }) == true { return true }
        guard !offline else { return false }
        try await loadThreadConversation(id: threadId, target: parentId)
        return threadMessages[threadId]?.contains { $0.id == parentId } == true
    }

    func hydrateThread(id: String, target: String?, duringRefresh: Bool = false) async throws {
        guard duringRefresh || !offline else { return }
        let view = try await loadThread(id: id)
        if rootMessage(view.thread.rootMessageId) == nil { _ = try await loadThreadRoot(id: id) }
        try await loadThreadConversation(id: id, target: target)
    }

    func loadNewerThreadMessages(id: String) async throws {
        let expected = generation
        let values = try await activeService().threadMessages(id: id,
            after: threadMessages[id]?.last?.id ?? "")
        try check(expected)
        guard values.allSatisfy({ $0.threadId == id }) else { throw DenFailure.invalidResponse }
        mergeThreadMessages(values, threadId: id)
        threadHasNewer[id] = values.count == 100
        saveCache()
    }

    func mergeThreadMessages(_ values: [API.Message], threadId: String) {
        var byId = Dictionary(uniqueKeysWithValues: (threadMessages[threadId] ?? []).map { ($0.id, $0) })
        for message in values where message.threadId == threadId { byId[message.id] = message }
        threadMessages[threadId] = byId.values.sorted { $0.id < $1.id }
    }

    func mergeMessage(_ message: API.Message) {
        if let threadId = message.threadId { mergeThreadMessages([message], threadId: threadId) }
        else {
            let roomContainsMessage = messages[message.channelId]?.contains { $0.id == message.id } == true
            let cachedAsThreadRoot = threadRoots[message.id] != nil
            if roomContainsMessage || !cachedAsThreadRoot { merge([message], channelId: message.channelId) }
            mergeInlineThread(message)
            if cachedAsThreadRoot || threadMetadata.values.contains(where: { $0.rootMessageId == message.id }) {
                threadRoots[message.id] = message
            }
        }
    }

    func applyThreadMetadata(_ summary: API.ThreadSummary, token: Int? = nil) {
        guard metadataOrder.accept(.thread(summary.id), token: token) else { return }
        threadMetadata[summary.id] = summary
        bindSelection(to: summary)
    }

    func applyThreadMetadata(_ summary: API.ThreadSummary, snapshot: [ReadKey: Int]) {
        guard metadataOrder.accept(.thread(summary.id), from: snapshot) else { return }
        threadMetadata[summary.id] = summary
        bindSelection(to: summary)
    }

    func applyThreadRead(_ state: API.ThreadReadState, token: Int? = nil) {
        guard readOrder.accept(.thread(state.threadId), token: token) else { return }
        threadReadStates[state.threadId] = state
    }

    func applyThreadRead(_ state: API.ThreadReadState, snapshot: [ReadKey: Int]) {
        guard readOrder.accept(.thread(state.threadId), from: snapshot) else { return }
        threadReadStates[state.threadId] = state
    }

    func mergeInlineThread(_ message: API.Message) {
        guard let value = message.thread else { return }
        if let existing = threadMetadata[value.id] {
            bindSelection(to: existing)
            return
        }
        applyThreadMetadata(.init(channelId: value.channelId, createdAt: value.createdAt,
            createdBy: value.createdBy, id: value.id, lastActivityAt: value.lastActivityAt,
            lastReplyId: value.lastReplyId, replyCount: value.replyCount,
            resolvedAt: value.resolvedAt, resolvedBy: value.resolvedBy,
            rootMessageId: value.rootMessageId, title: value.title))
    }

    func markThreadRead(id: String, messageId: String) async throws {
        let expected = generation
        let token = readOrder.begin(.thread(id))
        let state = try await activeService().markThreadRead(id: id, messageId: messageId)
        try check(expected)
        applyThreadRead(state, token: token)
        try await refreshChannelRead(channelId: state.channelId)
        saveCache(); await notifications?.updateBadge()
    }

    func followThread(id: String, following: Bool) async throws {
        let expected = generation
        let token = readOrder.begin(.thread(id))
        let state = try await activeService().followThread(id: id, following: following)
        try check(expected)
        applyThreadRead(state, token: token)
        try await refreshChannelRead(channelId: state.channelId)
        saveCache(); await notifications?.updateBadge()
    }

    @discardableResult
    func updateThread(id: String, title: String? = nil, resolved: Bool? = nil) async throws -> API.ThreadSummary {
        let expected = generation
        let token = metadataOrder.begin(.thread(id))
        let summary = try await activeService().updateThread(id: id, title: title, resolved: resolved)
        try check(expected)
        guard summary.id == id else { throw DenFailure.invalidResponse }
        applyThreadMetadata(summary, token: token)
        saveCache()
        return summary
    }

    func refreshThreadStates(channelId: String) async throws {
        let ids = threadMetadata.values.filter { $0.channelId == channelId }.map(\.id)
        for id in ids { _ = try await loadThread(id: id) }
    }

    private func refreshChannelRead(channelId: String) async throws {
        let expected = generation
        let values = try await activeService().readStates()
        try check(expected)
        if let state = values.first(where: { $0.channelId == channelId }) { applyChannelRead(state) }
    }

    func resetThreads() {
        threadMetadata = [:]; threadReadStates = [:]; threadMessages = [:]
        threadRoots = [:]; threadHasNewer = [:]
        metadataOrder.reset()
    }

    private func bindSelection(to summary: API.ThreadSummary) {
        guard selectedThread?.channelId == summary.channelId,
              selectedThread?.threadId == summary.id || selectedThread?.rootId == summary.rootMessageId else { return }
        selectedThread?.rootId = summary.rootMessageId
        selectedThread?.threadId = summary.id
    }
}
