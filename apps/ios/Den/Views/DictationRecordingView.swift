import SwiftUI

/// T3 Code's recording interaction, expressed with native controls: cancel,
/// actual input-level history and elapsed time, then an explicit finish action.
struct DictationRecordingView: View {
    let controller: DictationController
    let theme: DenTheme
    let onCancel: () -> Void
    let onFinish: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var levels = Array(repeating: 0.0, count: 64)

    private var recording: Bool { controller.state == .listening }
    private var elapsed: String {
        let seconds = max(0, Int(controller.recordingDuration))
        return "\(seconds / 60):" + String(format: "%02d", seconds % 60)
    }

    var body: some View {
        HStack(spacing: 8) {
            Button(action: onCancel) {
                Image(systemName: "xmark").font(.system(size: 20))
                    .frame(width: 44, height: 44).contentShape(Rectangle())
            }
            .accessibilityLabel("Cancel dictation")
            .accessibilityIdentifier("composer-dictation-cancel")

            Group {
                if recording {
                    HStack(spacing: 10) {
                        GeometryReader { geometry in
                            let count = max(1, min(64, Int(geometry.size.width / 5)))
                            HStack(spacing: 3) {
                                ForEach(0..<count, id: \.self) { index in
                                    let level = levels[64 - count + index]
                                    Capsule().fill(theme.ink.opacity(0.22 + 0.78 * level))
                                        .frame(width: 2, height: 2 + 30 * level)
                                }
                            }
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
                            .animation(reduceMotion ? nil : .easeOut(duration: 0.1), value: levels)
                        }
                        .frame(height: 32).clipped().accessibilityHidden(true)
                        Text(elapsed).font(theme.monoFont(.caption)).monospacedDigit()
                            .foregroundStyle(theme.ink2).fixedSize()
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Recording")
                    .accessibilityValue(String(format: "%.1f seconds", controller.recordingDuration))
                    .accessibilityIdentifier("dictation-recording")
                } else {
                    Text(controller.state == .finishing ? "Transcribing…" : "Preparing…")
                        .font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityIdentifier("dictation-recording-status")
                }
            }
            .frame(maxWidth: .infinity)

            Button(action: onFinish) {
                Group {
                    if recording {
                        Image(systemName: "checkmark").font(.system(size: 16, weight: .semibold))
                            .foregroundStyle(theme.bg)
                            .frame(width: 30, height: 30).background(theme.accent, in: Circle())
                    } else { ProgressView().tint(theme.ink2).frame(width: 30, height: 30) }
                }
                .frame(width: 44, height: 44).contentShape(Rectangle())
            }
            .disabled(!recording)
            .accessibilityLabel("Finish dictation")
            .accessibilityIdentifier("composer-dictation-finish")
        }
        .buttonStyle(.plain).foregroundStyle(theme.ink2)
        .frame(height: 44)
        .onChange(of: controller.recordingDuration) { _, _ in
            guard recording else { return }
            let amplitude = max(0, min(1, controller.audioLevel))
            levels.removeFirst()
            levels.append(amplitude <= 0.001 ? 0 : sqrt(amplitude))
        }
    }
}

/// Keeps one UITextView mounted as the compact row expands. A multiline draft
/// uses the full width above the controls instead of squeezing between icons.
struct ComposerLayout: Layout {
    let expanded: Bool
    let trailingWidth: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 320
        let textWidth = max(1, width - (expanded ? 16 : 48 + trailingWidth))
        let textHeight = subviews[0].sizeThatFits(.init(width: textWidth, height: nil)).height
        return CGSize(width: width, height: expanded ? textHeight + 44 : 44)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let textWidth = max(1, bounds.width - (expanded ? 16 : 48 + trailingWidth))
        let textHeight = subviews[0].sizeThatFits(.init(width: textWidth, height: nil)).height
        subviews[0].place(at: CGPoint(x: bounds.minX + (expanded ? 8 : 48), y: bounds.minY),
                          proposal: .init(width: textWidth, height: expanded ? textHeight : 44))
        subviews[1].place(at: CGPoint(x: bounds.minX, y: bounds.minY + (expanded ? textHeight : 0)),
                          proposal: .init(width: bounds.width, height: 44))
    }
}
