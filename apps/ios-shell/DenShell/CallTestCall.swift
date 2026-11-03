#if DEBUG && targetEnvironment(simulator)
import Foundation
import UIKit

// A made-up call for the UI check (launch with `-DenFakeCall "<ellis id>,<maya id>,<theo id>,<juno id>"`). Debug
// simulator builds only: none of this is compiled into a phone build.
//
// Xcode Cloud's simulator can't carry call media: its only network is a VPN tunnel, and nothing on the machine answers a
// socket bound to it (report 48). So this answers the page's call commands itself, as the engine would once connected:
// you (Ellis), Maya with her camera on and talking, Theo sharing a window, Juno muted. Every video slot gets a test
// pattern in place of LiveKit's picture, so the check shows the real path: the page's slots, the views behind the web
// view, the holes, the names on top, a sheet over them, and the video going with the call view.

@MainActor final class FakeCall {
    static let shared: FakeCall? = UserDefaults.standard.string(forKey: "DenFakeCall").flatMap(FakeCall.init)

    private let ids: [String]
    private var revision = 1_000
    private var call: (channelId: String, micOn: Bool, micSeq: Int, soundOff: Bool, soundSeq: Int, cameraOn: Bool, cameraSeq: Int)?

    private init?(_ raw: String) {
        ids = raw.split(separator: ",").map(String.init)
        guard ids.count == 4 else { return nil }
    }

    func answer(_ command: CallCommand) -> String {
        switch command {
        case .join(let join): call = (join.channelId, join.micOn, 0, false, 0, false, 0)
        case .leave: call = nil
        case .mute(let muted, let seq): call?.micOn = !muted; call?.micSeq = seq
        case .sound(let off, let seq): call?.soundOff = off; call?.soundSeq = seq
        case .camera(let on, let seq): call?.cameraOn = on; call?.cameraSeq = seq
        case .flip, .state: break
        }
        revision += 1
        guard let call else { return "{\"revision\":\(revision)}" }
        func person(_ at: Int, _ name: String, local: Bool = false, speaking: Bool = false, muted: Bool = false, camera: Bool = false,
                    screens: [CallSnapshot.Screen] = []) -> CallSnapshot.Person {
            .init(identity: "\(ids[at]):fake", name: name, local: local, speaking: speaking, muted: muted, quality: "excellent",
                  camera: camera, screen: !screens.isEmpty, screens: screens)
        }
        let people = [
            person(0, "Ellis", local: true, muted: !call.micOn, camera: call.cameraOn),
            person(1, "Maya", speaking: true, camera: true),
            person(2, "Theo", screens: [.init(name: "screen-1", label: "Cabin plan.pdf", surface: "window")]),
            person(3, "Juno", muted: true),
        ]
        let snapshot = CallSnapshot(revision: revision, phase: .connected, channelId: call.channelId, micSeq: call.micSeq, soundSeq: call.soundSeq,
                                    cameraSeq: call.cameraSeq, micOn: call.micOn, cameraOn: call.cameraOn, soundOff: call.soundOff,
                                    otherDevices: 0, error: nil, participants: people)
        return (try? CallBridge.json(snapshot)) ?? "{\"revision\":\(revision)}"
    }
}

extension VideoSlotView {
    /// In place of a picture from LiveKit: a gradient (warm for a camera, cool for a share) that says what it stands for.
    func showTestPattern(_ key: String) {
        guard viewWithTag(4_242) == nil else { return }
        let share = key.contains("|screen|")
        let pattern = UIView(frame: bounds)
        pattern.tag = 4_242
        pattern.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        let gradient = CAGradientLayer()
        gradient.colors = (share ? [UIColor.systemBlue, .systemIndigo] : [UIColor.systemOrange, .systemGreen]).map(\.cgColor)
        gradient.startPoint = .zero
        gradient.endPoint = CGPoint(x: 1, y: 1)
        gradient.frame = CGRect(x: 0, y: 0, width: 2_000, height: 2_000)
        pattern.layer.addSublayer(gradient)
        pattern.clipsToBounds = true
        let label = UILabel()
        label.text = share ? "Native share" : "Native camera"
        label.textColor = .white
        label.font = .preferredFont(forTextStyle: .title2)
        label.translatesAutoresizingMaskIntoConstraints = false
        pattern.addSubview(label)
        addSubview(pattern)
        NSLayoutConstraint.activate([label.centerXAnchor.constraint(equalTo: pattern.centerXAnchor), label.centerYAnchor.constraint(equalTo: pattern.centerYAnchor)])
        markDrawn()
    }
}
#endif
