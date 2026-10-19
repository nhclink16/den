import Foundation
import Testing
@testable import DenShell

// The page names a haptic and the shell plays it. Only the few names Den uses may pass, exactly as written.
struct HapticsTests {
    @Test func readsTheKindsDenUses() {
        #expect(Haptic("light") == .light)
        #expect(Haptic("medium") == .medium)
        #expect(Haptic("selection") == .selection)
        #expect(Haptic("warning") == .warning)
        for kind in Haptic.allCases { #expect(Haptic(kind.rawValue) == kind) }
    }

    @Test func refusesAnythingElse() {
        #expect(Haptic("heavy") == nil, "a real iOS style, but not one Den uses")
        #expect(Haptic("success") == nil)
        #expect(Haptic("Light") == nil)
        #expect(Haptic(" light") == nil)
        #expect(Haptic("") == nil)
        #expect(Haptic(1) == nil)
        #expect(Haptic(true) == nil)
        #expect(Haptic(["kind": "light"]) == nil)
        #expect(Haptic(NSNull()) == nil)
    }
}
