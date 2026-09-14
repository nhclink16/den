import LiveKit
import SwiftUI

/// Place once above the app's tab/navigation content with a bottom safe-area inset.
/// requestMute receives CallKit isMuted, not microphone-is-enabled. Dismissal never leaves.
struct CallDock: View {
    let session: CallSession
    let theme: DenTheme
    let requestMute: (Bool) -> Void
    let requestEnd: () -> Void
    @State private var expanded = false

    var body: some View {
        if session.isActive {
            HStack(spacing: 8) {
                Button { expanded = true } label: {
                    HStack(spacing: 12) {
                        if session.pictureInPicture.hasVideo {
                            CallPiPThumbnail(owner: session.pictureInPicture).frame(width: 56, height: 40)
                                .clipShape(RoundedRectangle(cornerRadius: theme.smallRadius))
                        } else { Image(systemName: "waveform").foregroundStyle(theme.accent) }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(session.title).font(theme.bodyFont(.subheadline).weight(.semibold)).lineLimit(1)
                            Text(session.status).font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                        }
                        Spacer(minLength: 0)
                    }.frame(minHeight: 44).contentShape(Rectangle())
                }
                .buttonStyle(.plain).accessibilityLabel("Open call in \(session.title)")
                .accessibilityValue(session.status).accessibilityIdentifier("call-dock-open")
                Button { requestMute(session.wantsMicrophone) } label: {
                    Image(systemName: session.wantsMicrophone ? "mic.fill" : "mic.slash.fill")
                        .frame(width: 44, height: 44)
                }
                .buttonStyle(.plain).disabled(!session.isConnected)
                .accessibilityLabel(session.wantsMicrophone ? "Mute microphone" : "Unmute microphone")
                .accessibilityIdentifier("call-dock-microphone")
                Button(role: .destructive, action: requestEnd) {
                    Image(systemName: "phone.down.fill").frame(width: 44, height: 44)
                }
                .tint(theme.danger).accessibilityLabel("Leave call")
                .accessibilityIdentifier("call-dock-leave")
            }
            .padding(.horizontal, 12).padding(.vertical, 6).foregroundStyle(theme.ink)
            .background(theme.bg2).overlay(alignment: .top) { theme.line.frame(height: 1) }
            .sheet(isPresented: $expanded) {
                CallMediaView(session: session, theme: theme, requestMute: requestMute, requestEnd: requestEnd)
                    .onAppear { session.pictureInPicture.restoredUserInterface(for: session.callID) }
            }
            .onChange(of: session.callID) { _, id in if id == nil { expanded = false } }
            .onAppear {
                session.pictureInPicture.onRestoreUserInterface = {
                    if expanded { session.pictureInPicture.restoredUserInterface(for: session.callID) }
                    else { expanded = true }
                }
            }
            .onDisappear { session.pictureInPicture.onRestoreUserInterface = nil }
        }
    }
}

struct CallMediaView: View {
    let session: CallSession
    let theme: DenTheme
    let requestMute: (Bool) -> Void
    let requestEnd: () -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(session.status).font(theme.bodyFont(.subheadline))
                        Spacer()
                        Text("\(session.people.count) \(session.people.count == 1 ? "person" : "people")")
                            .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                    }.accessibilityElement(children: .combine)
                    if let error = session.error {
                        VStack(alignment: .leading, spacing: 8) {
                            Label(error, systemImage: "exclamationmark.circle")
                                .font(theme.bodyFont(.callout))
                            Button("Dismiss message") { session.clearError() }.frame(minHeight: 44)
                        }.padding(12).background(theme.bg3, in: RoundedRectangle(cornerRadius: theme.radius))
                    }
                    if session.joinedQuietly {
                        Label("You joined with microphone and sound off because another device is in this call.", systemImage: "iphone.and.arrow.forward")
                            .font(theme.bodyFont(.footnote)).foregroundStyle(theme.ink2)
                    }
                    media
                    controls
                    people
                }.padding(16)
            }
            .background(theme.bg).foregroundStyle(theme.ink).tint(theme.accent)
            .navigationTitle(session.title).navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Back to chat") { dismiss() }.accessibilityIdentifier("call-back-to-chat")
                }
                if session.pictureInPicture.hasVideo {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button { session.pictureInPicture.start() } label: { Image(systemName: "pip.enter") }
                            .disabled(!session.pictureInPicture.canStart)
                            .accessibilityLabel("Picture in Picture")
                    }
                }
            }
        }
    }

    private var media: some View {
        VStack(spacing: 12) {
            // Every share has its own full-width tile. Cameras are not replaced by a share.
            ForEach(session.videos.filter(\.isShare)) { video in tile(video) }
            let cameras = session.videos.filter { !$0.isShare }
            if !cameras.isEmpty {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: dynamicTypeSize.isAccessibilitySize ? 280 : 156), spacing: 12)], spacing: 12) {
                    ForEach(cameras) { video in tile(video) }
                }
            } else if session.videos.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "waveform").font(.system(size: 40)).foregroundStyle(theme.accent).accessibilityHidden(true)
                    Text("Here for the conversation").font(theme.bodyFont(.headline))
                    Text("Cameras and screen shares appear here.").font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2)
                }.frame(maxWidth: .infinity).padding(.vertical, 28)
            }
        }
    }

    private func tile(_ video: CallVideo) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack {
                theme.bg3
                if let track = video.track, !video.isMuted {
                    CallTrackView(track: track, isShare: video.isShare, isLocal: video.isLocal)
                        .accessibilityHidden(true)
                } else {
                    VStack(spacing: 8) {
                        Image(systemName: video.isShare ? "rectangle.on.rectangle" : "video.slash")
                        Text(video.isMuted ? (video.isShare ? "Share paused" : "Camera paused") : "Waiting for video")
                            .font(theme.bodyFont(.caption)).multilineTextAlignment(.center)
                    }.padding(12).foregroundStyle(theme.ink2)
                }
            }
            .aspectRatio(video.isShare ? 16 / 9 : 4 / 3, contentMode: .fit)
            .clipShape(RoundedRectangle(cornerRadius: theme.radius))
            Text("\(video.name)\(video.isLocal ? " · You" : "") · \(video.isShare ? "Screen" : "Camera")")
                .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(video.name), \(video.isShare ? "screen share" : "camera")\(video.isMuted ? ", paused" : "")")
        .accessibilityIdentifier("call-video-\(video.id)")
    }

    private var controls: some View {
        VStack(spacing: 12) {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), spacing: 10)], spacing: 10) {
                control(session.wantsMicrophone ? "Mute mic" : "Unmute mic",
                        icon: session.wantsMicrophone ? "mic.fill" : "mic.slash.fill", id: "call-microphone") {
                    requestMute(session.wantsMicrophone)
                }
                control(session.outputMuted ? "Sound on" : "Sound off",
                        icon: session.outputMuted ? "speaker.slash.fill" : "speaker.wave.2.fill", id: "call-sound") {
                    perform { try await session.setOutputMuted(!session.outputMuted) }
                }
                control(session.wantsCamera ? "Camera off" : "Camera on",
                        icon: session.wantsCamera ? "video.fill" : "video.slash.fill", id: "call-camera") {
                    perform { try await session.setCameraEnabled(!session.wantsCamera) }
                }
                if session.cameraEnabled {
                    control("Flip camera", icon: "arrow.triangle.2.circlepath.camera", id: "call-flip-camera") {
                        perform { try await session.switchCamera() }
                    }
                }
            }.disabled(!session.isConnected)
            Menu {
                Button("Automatic") { perform { try session.selectRoute(.system) } }
                Button("Speaker") { perform { try session.selectRoute(.speaker) } }
                ForEach(session.inputs) { input in
                    Button(input.name) { perform { try session.selectRoute(.input(input.id)) } }
                }
            } label: {
                Label(session.routeLabel, systemImage: "speaker.wave.2")
                    .font(theme.bodyFont(.subheadline)).frame(maxWidth: .infinity, minHeight: 44)
                    .padding(.horizontal, 12).background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.smallRadius))
            }
            .disabled(!session.audioActivated).accessibilityLabel("Audio route")
            .accessibilityValue(session.routeLabel).accessibilityIdentifier("call-audio-route")
            Button(role: .destructive, action: requestEnd) {
                Label("Leave call", systemImage: "phone.down.fill")
                    .font(theme.bodyFont(.headline)).frame(maxWidth: .infinity, minHeight: 48)
            }.buttonStyle(.bordered).tint(theme.danger).accessibilityIdentifier("call-leave")
        }
    }

    private func control(_ text: String, icon: String, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(text, systemImage: icon).font(theme.bodyFont(.subheadline))
                .frame(maxWidth: .infinity, minHeight: 48).padding(.horizontal, 6)
                .background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.smallRadius))
        }.buttonStyle(.plain).accessibilityIdentifier(id)
    }

    private var people: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("In this call").font(theme.bodyFont(.headline)).accessibilityAddTraits(.isHeader)
            ForEach(session.people) { person in
                HStack(spacing: 12) {
                    Image(systemName: person.isSpeaking ? "waveform" : "person.fill")
                        .foregroundStyle(person.isSpeaking ? theme.accent : theme.ink3).frame(width: 24)
                        .accessibilityHidden(true)
                    Text(person.name + (person.isYou ? " · You" : "")).font(theme.bodyFont(.subheadline))
                    Spacer()
                    if person.deviceCount > 1 {
                        Text("\(person.deviceCount) devices").font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                    }
                }
                .accessibilityElement(children: .combine)
                .accessibilityValue(person.isSpeaking ? "Speaking" : "")
            }
        }
    }

    private func perform(_ operation: @escaping @MainActor () async throws -> Void) {
        let expected = session.callID
        Task { @MainActor in
            do { try await operation() }
            catch is CancellationError { }
            catch {
                guard session.callID == expected else { return }
                session.error = (error as? CallSession.Failure)?.localizedDescription ?? CallSession.Failure.media.localizedDescription
            }
        }
    }
}

/// The same public renderer can also be hosted by the parent's dedicated PiP controller.
struct CallTrackView: UIViewRepresentable {
    let track: VideoTrack
    var isShare = false
    var isLocal = false

    func makeUIView(context: Context) -> LiveKit.VideoView {
        let view = LiveKit.VideoView()
        view.renderMode = .sampleBuffer
        updateUIView(view, context: context)
        return view
    }
    func updateUIView(_ view: LiveKit.VideoView, context: Context) {
        view.track = track
        view.layoutMode = isShare ? .fit : .fill
        view.mirrorMode = isLocal && !isShare ? .auto : .off
    }
    static func dismantleUIView(_ view: LiveKit.VideoView, coordinator: ()) { view.track = nil }
}
