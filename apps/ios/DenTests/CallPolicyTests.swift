import Testing
#if canImport(Den)
@testable import Den
#else
@testable import CallPolicyHarness
#endif

struct CallPolicyTests {
    @Test func countsPeopleWithoutCollapsingAccountPrefixesOrEmptyIdentities() {
        #expect(Set(["ann:phone", "ann:desktop", "anna:phone", "legacy", "", ":broken"].compactMap(CallPolicy.accountID)) == ["ann", "anna", "legacy"])
        #expect(CallPolicy.accountID("ann:device:retry") == "ann")
    }

    @Test func onlyAnAdditionalConnectionForThisAccountStartsQuietly() {
        #expect(CallPolicy.joinsQuietly(accountID: "ann", localIdentity: "ann:new", remoteIdentities: ["ann:existing", "anna:phone"]))
        #expect(!CallPolicy.joinsQuietly(accountID: "ann", localIdentity: "ann:new", remoteIdentities: ["ann:new", "anna:phone", ""]))
        #expect(CallPolicy.joinsQuietly(accountID: "ann", localIdentity: "ann:new", remoteIdentities: ["ann"]))
    }

    @Test func ownMicrophoneIsSuppressedButShareAudioAndVideoStayIndependent() {
        for muted in [false, true] {
            #expect(!CallPolicy.shouldSubscribe(isAudio: true, isMicrophone: true, identity: "ann:desktop", accountID: "ann", outputMuted: muted))
            #expect(CallPolicy.shouldSubscribe(isAudio: true, isMicrophone: false, identity: "ann:desktop", accountID: "ann", outputMuted: muted) == !muted)
            #expect(CallPolicy.shouldSubscribe(isAudio: true, isMicrophone: true, identity: "anna:phone", accountID: "ann", outputMuted: muted) == !muted)
            #expect(CallPolicy.shouldSubscribe(isAudio: false, isMicrophone: false, identity: "ann:desktop", accountID: "ann", outputMuted: muted))
        }
    }
}
