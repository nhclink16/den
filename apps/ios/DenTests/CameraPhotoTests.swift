import Testing
import UIKit
@testable import Den

struct CameraPhotoTests {
    @Test @MainActor func confirmedPhotoCreatesAnOwnedJPEGThatSurvivesAttachmentCopy() throws {
        let photo = UIGraphicsImageRenderer(size: CGSize(width: 32, height: 24)).image { context in
            UIColor.red.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 32, height: 24))
        }
        var result: Result<URL, Error>?
        let coordinator = CameraPhotoPicker.Coordinator { result = $0 }
        coordinator.imagePickerController(UIImagePickerController(), didFinishPickingMediaWithInfo: [.originalImage: photo])
        let captured = try #require(result).get()
        defer { AttachmentFiles.remove(captured) }
        let owned = try AttachmentFiles.copy(captured, id: UUID())
        defer { AttachmentFiles.remove(owned) }
        AttachmentFiles.remove(captured)
        let decoded = try #require(UIImage(contentsOfFile: owned.path))
        #expect(owned.pathExtension == "jpg")
        #expect(decoded.size.width > decoded.size.height)
        #expect(try AttachmentFiles.size(owned) > 0)
    }

    @Test @MainActor func cancellingDoesNotCreateAnAttachmentAndMissingPhotoReportsFailure() {
        var cancelled = false
        let coordinator = CameraPhotoPicker.Coordinator { result in
            cancelled = result == nil
            if case .success = result { Issue.record("Missing photo must not create an attachment") }
        }
        coordinator.imagePickerControllerDidCancel(UIImagePickerController())
        #expect(cancelled)
        coordinator.imagePickerController(UIImagePickerController(), didFinishPickingMediaWithInfo: [:])
        #expect(!cancelled)
    }
}
