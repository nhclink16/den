import Foundation

/// Analyzer results are per audio range; legacy results are cumulative for the request.
struct DictationTranscript {
    private struct Segment { let start: Double; let end: Double; let text: String }
    private var finalized: [Segment] = []
    private var volatile: Segment?

    mutating func analyzer(text: String, start: Double, end: Double, isFinal: Bool) -> String {
        guard start.isFinite, end.isFinite, end >= start else { return value }
        let segment = Segment(start: start, end: end, text: text)
        if isFinal {
            if let index = finalized.firstIndex(where: { $0.start == start && $0.end == end }) {
                finalized[index] = segment
            } else if end > (finalized.last?.end ?? -.infinity) {
                finalized.append(segment)
            }
            if let current = volatile, current.start < end || current.end <= end { volatile = nil }
        } else if end > (finalized.last?.end ?? -.infinity) {
            volatile = segment
        }
        return value
    }

    static func legacy(_ text: String) -> String { text.trimmingCharacters(in: .whitespacesAndNewlines) }
    var value: String {
        // SpeechTranscriber supplies inter-segment whitespace. Don't invent Latin spaces
        // for languages whose writing system doesn't separate every word with spaces.
        (finalized.map(\.text).joined() + (volatile?.text ?? "")).trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
