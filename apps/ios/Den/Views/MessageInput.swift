import SwiftUI
import UIKit

/// A multiline native editor with chat Return semantics. Pasted multiline text and
/// input-method composition are preserved; Shift-Return explicitly inserts a line.
struct MessageInput: UIViewRepresentable {
    @Binding var text: String
    @Binding var focused: Bool
    let theme: DenTheme
    let onSend: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> ChatTextView {
        let view = ChatTextView()
        view.delegate = context.coordinator
        view.backgroundColor = .clear
        view.textContainerInset = UIEdgeInsets(top: 11, left: 0, bottom: 11, right: 0)
        view.textContainer.lineFragmentPadding = 0
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        view.returnKeyType = .send
        view.adjustsFontForContentSizeCategory = true
        view.accessibilityLabel = "Message"
        view.accessibilityHint = "Return sends. Shift-Return adds a new line."
        view.accessibilityIdentifier = "composer-field"
        let toolbar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
        let done = UIBarButtonItem(title: "Done", style: .plain, target: view, action: #selector(ChatTextView.dismissKeyboard))
        done.accessibilityLabel = "Dismiss keyboard"
        done.accessibilityIdentifier = "composer-dismiss-keyboard"
        toolbar.items = [.flexibleSpace(), done]
        view.inputAccessoryView = toolbar
        return view
    }

    func updateUIView(_ view: ChatTextView, context: Context) {
        context.coordinator.parent = self
        if view.text != text { view.text = text }
        view.font = DenFonts.uiFont(theme.family.fonts.body, compatibleWith: view.traitCollection)
        view.textColor = UIColor(theme.ink)
        view.tintColor = UIColor(theme.accent)
        if let toolbar = view.inputAccessoryView as? UIToolbar {
            toolbar.tintColor = UIColor(theme.accent)
        }
        if focused && !view.isFirstResponder { view.becomeFirstResponder() }
        else if !focused && view.isFirstResponder { view.resignFirstResponder() }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: ChatTextView, context: Context) -> CGSize? {
        guard let width = proposal.width, width > 0 else { return nil }
        let natural = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
        let maximum = (uiView.font?.lineHeight ?? 22) * 6 + 22
        return CGSize(width: width, height: max(44, min(natural.height, maximum)))
    }

    @MainActor final class Coordinator: NSObject, UITextViewDelegate {
        var parent: MessageInput
        init(_ parent: MessageInput) { self.parent = parent }
        func textViewDidChange(_ textView: UITextView) { parent.text = textView.text }
        func textViewDidBeginEditing(_ textView: UITextView) { parent.focused = true }
        func textViewDidEndEditing(_ textView: UITextView) { parent.focused = false }
        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            if text == "\n", textView.markedTextRange == nil,
               (textView as? ChatTextView)?.insertingLineBreak != true {
                parent.onSend()
                return false
            }
            return true
        }
    }
}

final class ChatTextView: UITextView {
    var insertingLineBreak = false
    override var keyCommands: [UIKeyCommand]? {
        let newline = UIKeyCommand(input: "\r", modifierFlags: .shift, action: #selector(insertLineBreak))
        newline.discoverabilityTitle = "New line"
        newline.wantsPriorityOverSystemBehavior = true
        return (super.keyCommands ?? []) + [newline]
    }
    @objc private func insertLineBreak() {
        insertingLineBreak = true
        defer { insertingLineBreak = false }
        insertText("\n")
    }
    @objc func dismissKeyboard() { resignFirstResponder() }
}
