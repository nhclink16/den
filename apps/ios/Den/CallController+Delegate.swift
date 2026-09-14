import AVFoundation
import CallKit

extension CallController: @preconcurrency CXProviderDelegate {
    // CXProvider was explicitly assigned DispatchQueue.main. Do not insert an async hop
    // before associating activation with the UUID armed by the completed start/answer action.
    func providerDidReset(_ provider: CXProvider) { reset() }
    func provider(_ provider: CXProvider, perform action: CXStartCallAction) { start(action) }
    func provider(_ provider: CXProvider, perform action: CXAnswerCallAction) { answer(action) }
    func provider(_ provider: CXProvider, perform action: CXEndCallAction) { end(action) }
    func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) { mute(action) }
    func provider(_ provider: CXProvider, timedOutPerforming action: CXAction) { timedOut(action) }
    func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) { activated() }
    func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) { deactivated() }
}
