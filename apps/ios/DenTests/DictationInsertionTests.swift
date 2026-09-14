import Foundation
import Testing
@testable import Den

struct DictationInsertionTests {
    @Test func fullTranscriptRevisionsReplaceOnlyTheSessionSpan() throws {
        let original = "Meet today."
        var insertion = try #require(DictationInsertion(text: original, selection: NSRange(location: 5, length: 0)))
        var current = original
        for (transcript, expected) in [
            ("Alice is", "Meet Alice is today."),
            ("Alice was", "Meet Alice was today."),
            ("Alice was ready. We sh", "Meet Alice was ready. We sh today."),
            ("Alice was ready. We should go.", "Meet Alice was ready. We should go. today."),
            ("Alice was ready. We should go.", "Meet Alice was ready. We should go. today."),
            ("", original),
            ("Bob", "Meet Bob today."),
        ] {
            let result = insertion.update(transcript: transcript, currentText: current)
            let update = try #require(result)
            #expect(update.text == expected)
            current = update.text
        }
    }

    @Test func unicodeCaretAndSelectedTextArePreserved() throws {
        // The emoji occupies seven UTF-16 units; the decomposed accent occupies two.
        let original = "👩🏽‍💻 cafe\u{301} 東京!"
        var insertion = try #require(DictationInsertion(text: original, selection: NSRange(location: 14, length: 2)))
        let firstResult = insertion.update(transcript: "naïve 🐈", currentText: original)
        let first = try #require(firstResult)
        #expect(Array(first.text.utf16) == Array("👩🏽‍💻 cafe\u{301} naïve 🐈 東京!".utf16))
        #expect(first.selection == NSRange(location: 22, length: 0))
        let revisedResult = insertion.update(transcript: "déjà vu", currentText: first.text)
        let revised = try #require(revisedResult)
        #expect(Array(revised.text.utf16) == Array("👩🏽‍💻 cafe\u{301} déjà vu 東京!".utf16))
        #expect(revised.selection == NSRange(location: 21, length: 0))
    }

    @Test func externalEditsCancelEvenWhenUnicodeTextLooksEquivalent() throws {
        let original = "café later"
        var insertion = try #require(DictationInsertion(text: original, selection: NSRange(location: 5, length: 0)))
        let firstResult = insertion.update(transcript: "hello", currentText: original)
        let first = try #require(firstResult)
        let externallyEdited = insertion.update(transcript: "hello again", currentText: first.text + "!")
        #expect(externallyEdited == nil)
        let equivalentEdit = insertion.update(transcript: "hello again", currentText: "cafe\u{301} hello later")
        #expect(equivalentEdit == nil)
        // Rejected updates must not advance the expected composer state.
        let unchangedResult = insertion.update(transcript: "hello again", currentText: first.text)
        let unchanged = try #require(unchangedResult)
        #expect(unchanged.text == "café hello again later")
    }

    @Test func separatorsRespectWhitespacePunctuationAndEmptyResults() throws {
        let cases: [(String, Int, String, String, Int)] = [
            ("Hello", 5, "there", "Hello there", 11),
            ("Hello, friend", 5, "there", "Hello there, friend", 11),
            ("Hello ()", 7, "friend", "Hello (friend)", 13),
            ("Hello\nworld", 6, "there", "Hello\nthere world", 11),
            ("", 0, "  hello  ", "hello", 5),
            ("Hello world", 6, " \n ", "Hello world", 6),
        ]
        for (original, caret, transcript, expected, expectedCaret) in cases {
            var insertion = try #require(DictationInsertion(text: original, selection: NSRange(location: caret, length: 0)))
            let result = insertion.update(transcript: transcript, currentText: original)
            let update = try #require(result)
            #expect(update.text == expected)
            #expect(update.selection == NSRange(location: expectedCaret, length: 0))
        }
    }

    @Test func invalidOrSplitUnicodeSelectionsAreRejected() {
        let text = "🐈e\u{301}z"
        for range in [
            NSRange(location: NSNotFound, length: 0),
            NSRange(location: -1, length: 0),
            NSRange(location: 0, length: -1),
            NSRange(location: 6, length: 0),
            NSRange(location: 1, length: 0), // Inside a surrogate pair.
            NSRange(location: 3, length: 0), // Inside the decomposed character.
            NSRange(location: 2, length: 1), // Selection ends inside that character.
            NSRange(location: 4, length: Int.max),
        ] {
            #expect(DictationInsertion(text: text, selection: range) == nil, "Range: \(range)")
        }
    }
}
