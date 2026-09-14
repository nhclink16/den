import Foundation

/// Owns one dictation session's text and separators, never the surrounding draft.
struct DictationInsertion: Sendable {
    private let prefix: String
    private let suffix: String
    private var expectedText: String

    init?(text: String, selection: NSRange) {
        let length = text.utf16.count
        guard selection.location >= 0, selection.length >= 0,
              selection.location <= length, selection.length <= length - selection.location,
              let range = Range(selection, in: text) else { return nil }
        // Reject positions inside composed characters as well as invalid UTF-16.
        guard (range.lowerBound == text.endIndex || text.indices.contains(range.lowerBound)),
              (range.upperBound == text.endIndex || text.indices.contains(range.upperBound)) else { return nil }
        // A selected draft is not a deletion request. Insert before it and retain it.
        prefix = String(text[..<range.lowerBound])
        suffix = String(text[range.lowerBound...])
        expectedText = text
    }

    /// Pass the full session transcript, including its finalized prefix, not a delta.
    /// A nil result means the composer changed externally; the caller must stop dictation.
    mutating func update(transcript: String, currentText: String) -> (text: String, selection: NSRange)? {
        // String equality treats different Unicode encodings as equivalent. UITextView
        // ranges do not, so even a canonically equivalent manual edit cancels ownership.
        guard currentText.utf16.elementsEqual(expectedText.utf16) else { return nil }
        let spoken = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
        let leading = spoken.isEmpty ? "" : Self.separator(between: prefix.last, and: spoken.first)
        let trailing = spoken.isEmpty ? "" : Self.separator(between: spoken.last, and: suffix.first)
        let beforeCaret = prefix + leading + spoken
        let result = beforeCaret + trailing + suffix
        expectedText = result
        // Leave the caret after spoken text, before the preserved suffix's separator.
        return (result, NSRange(location: beforeCaret.utf16.count, length: 0))
    }

    private static func separator(between left: Character?, and right: Character?) -> String {
        guard let left, let right, !left.isWhitespace, !right.isWhitespace else { return "" }
        if "([{“‘".contains(left) || ".,!?;:)]}”’".contains(right) { return "" }
        return " "
    }
}
