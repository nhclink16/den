import LiveKit

enum CallRoomEvent: Sendable { case presentation, publications, reconnecting, reconnected, disconnected }

extension CallSession: RoomDelegate {
    nonisolated private func deliver(_ room: Room, _ event: CallRoomEvent) {
        Task { @MainActor [weak self] in self?.receive(room, event: event) }
    }
    nonisolated func room(_ room: Room, participantDidConnect participant: RemoteParticipant) { deliver(room, .publications) }
    nonisolated func room(_ room: Room, participantDidDisconnect participant: RemoteParticipant) { deliver(room, .publications) }
    nonisolated func room(_ room: Room, didUpdateSpeakingParticipants participants: [Participant]) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: Participant, didUpdateName name: String) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: LocalParticipant, didPublishTrack publication: LocalTrackPublication) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: LocalParticipant, didUnpublishTrack publication: LocalTrackPublication) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didPublishTrack publication: RemoteTrackPublication) { deliver(room, .publications) }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didUnpublishTrack publication: RemoteTrackPublication) { deliver(room, .publications) }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didSubscribeTrack publication: RemoteTrackPublication) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didUnsubscribeTrack publication: RemoteTrackPublication) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, participant: Participant, trackPublication: TrackPublication, didUpdateIsMuted isMuted: Bool) { deliver(room, .presentation) }
    nonisolated func room(_ room: Room, didStartReconnectWithMode reconnectMode: ReconnectMode) { deliver(room, .reconnecting) }
    nonisolated func room(_ room: Room, didCompleteReconnectWithMode reconnectMode: ReconnectMode) { deliver(room, .reconnected) }
    nonisolated func room(_ room: Room, didDisconnectWithError error: LiveKitError?) { deliver(room, .disconnected) }
}
