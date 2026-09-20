import DenAPI
import Foundation
import SwiftUI

/// Presentation decisions live here so timeline boundaries and mention handling can be tested.
enum MessagePresentation {
    struct Mention: Equatable, Sendable {
        let range: NSRange
        let userId: String
        let displayName: String
    }

    struct Block: Identifiable, Equatable, Sendable {
        let id: Int
        let text: String
        let isCode: Bool
    }

    static func date(_ timestamp: String) -> Date? {
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractional.date(from: timestamp) ?? ISO8601DateFormatter().date(from: timestamp)
    }

    static func groups(_ message: API.Message, after previous: API.Message?, calendar: Calendar = .current) -> Bool {
        guard let previous, previous.authorId == message.authorId,
              previous.channelId == message.channelId, message.replyTo == nil,
              let date = Self.date(message.createdAt), let prior = Self.date(previous.createdAt),
              calendar.isDate(date, inSameDayAs: prior) else { return false }
        return date.timeIntervalSince(prior) >= 0 && date.timeIntervalSince(prior) < 300
    }

    static func startsDay(_ message: API.Message, after previous: API.Message?, calendar: Calendar = .current) -> Bool {
        guard let previous, let date = Self.date(message.createdAt), let prior = Self.date(previous.createdAt) else { return true }
        return !calendar.isDate(date, inSameDayAs: prior)
    }

    static func firstUnread(in messages: [API.Message], after lastReadId: String?, excluding userId: String?) -> String? {
        messages.first { message in
            (lastReadId.map { $0 < message.id } ?? true) && message.authorId != userId
        }?.id
    }

    /// Backtick code fences stay literal, including names that happen to begin with @.
    static func blocks(_ source: String) -> [Block] {
        source.components(separatedBy: "```").enumerated().compactMap { index, part in
            guard !part.isEmpty else { return nil }
            let code = index % 2 == 1
            var text = part
            if code, let newline = text.firstIndex(of: "\n") {
                let language = text[..<newline]
                if language.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_" || $0 == "-") }) {
                    text = String(text[text.index(after: newline)...])
                }
            }
            if code, text.hasSuffix("\n") { text.removeLast() }
            return Block(id: index, text: text, isCode: code)
        }
    }

    /// Operates on a rendered plain-text run, after Markdown has excluded code and links.
    static func mentions(in text: String, users: [API.User]) -> [Mention] {
        guard let pattern = try? NSRegularExpression(pattern: "(?<![\\w])@([a-z0-9_]{3,32})(?![a-z0-9_])") else { return [] }
        let byName = Dictionary(users.map { ($0.username, $0) }, uniquingKeysWith: { first, _ in first })
        let source = text as NSString
        return pattern.matches(in: text, range: NSRange(location: 0, length: source.length)).compactMap { match in
            guard let user = byName[source.substring(with: match.range(at: 1))] else { return nil }
            return Mention(range: match.range, userId: user.id,
                           displayName: user.displayName.isEmpty ? user.username : user.displayName)
        }
    }

    @MainActor
    static func markdown(_ source: String, users: [API.User], theme: DenTheme) -> AttributedString {
        let parsed = (try? AttributedString(markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(source)
        var result = AttributedString()
        for run in parsed.runs {
            let original = parsed[run.range]
            guard run.link == nil, run.inlinePresentationIntent?.contains(.code) != true else {
                var literal = AttributedString(original)
                if run.inlinePresentationIntent?.contains(.code) == true {
                    literal.font = theme.monoFont(.body)
                    literal.backgroundColor = theme.bg3
                }
                result.append(literal)
                continue
            }
            let string = String(original.characters)
            let mentions = mentions(in: string, users: users)
            var cursor = string.startIndex
            for mention in mentions {
                guard let range = Range(mention.range, in: string) else { continue }
                var prefix = AttributedString(String(string[cursor..<range.lowerBound]))
                prefix.mergeAttributes(run.attributes)
                result.append(prefix)
                var pill = AttributedString("@" + mention.displayName)
                pill.mergeAttributes(run.attributes)
                pill.foregroundColor = theme.accent
                pill.backgroundColor = theme.accentGlow
                pill.font = theme.bodyFont().weight(.semibold)
                result.append(pill)
                cursor = range.upperBound
            }
            var suffix = AttributedString(String(string[cursor...]))
            suffix.mergeAttributes(run.attributes)
            result.append(suffix)
        }
        return result
    }
}
