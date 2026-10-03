import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Use the system camera's capture, retake and confirmation UI. Only a confirmed photo is attached.
struct CameraPhotoPicker: UIViewControllerRepresentable {
    let onFinish: (Result<URL, Error>?) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onFinish: onFinish) }

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.mediaTypes = [UTType.image.identifier]
        picker.cameraCaptureMode = .photo
        picker.delegate = context.coordinator
        picker.view.tintColor = .label
        return picker
    }

    func updateUIViewController(_ picker: UIImagePickerController, context: Context) {}

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let onFinish: (Result<URL, Error>?) -> Void

        init(onFinish: @escaping (Result<URL, Error>?) -> Void) { self.onFinish = onFinish }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { onFinish(nil) }

        func imagePickerController(_ picker: UIImagePickerController,
                                   didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            onFinish(Result {
                guard let photo = info[.originalImage] as? UIImage,
                      let data = photo.jpegData(compressionQuality: 0.9) else {
                    throw CocoaError(.fileWriteUnknown)
                }
                let directory = try AttachmentFiles.directory()
                let url = directory.appendingPathComponent("Photo.jpg")
                do {
                    try data.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
                    return url
                } catch {
                    AttachmentFiles.remove(url)
                    throw error
                }
            })
        }
    }
}
