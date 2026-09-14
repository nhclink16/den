import SwiftUI
import UIKit

/// A multiline native editor with chat Return semantics. Pasted multiline text and
/// input-method composition are preserved; Shift-Return explicitly inserts a line.
struct MessageInput: UIViewRepresentable {
    @Binding var text: String
    @Binding var focused: Bool
    @Binding var selection: NSRange
    let theme: DenTheme
    let onSend: () -> Void
    let onManualChange: () -> Void
    let onEscape: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> ChatTextView {
        let view = ChatTextView()
        view.delegate = context.coordinator
        view.onEscape = { context.coordinator.parent.onEscape() }
        view.onInteraction = { context.coordinator.parent.onManualChange() }
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
        context.coordinator.applyingUpdate = true
        defer { context.coordinator.applyingUpdate = false }
        if !view.text.utf16.elementsEqual(text.utf16) { view.text = text }
        if view.selectedRange != selection, Range(selection, in: text) != nil {
            view.selectedRange = selection
        }
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
        var applyingUpdate = false
        init(_ parent: MessageInput) { self.parent = parent }
        func textViewDidChange(_ textView: UITextView) {
            guard !applyingUpdate else { return }
            parent.onManualChange()
            parent.text = textView.text
            parent.selection = textView.selectedRange
        }
        func textViewDidChangeSelection(_ textView: UITextView) {
            guard !applyingUpdate, parent.selection != textView.selectedRange else { return }
            parent.onManualChange()
            parent.selection = textView.selectedRange
        }
        func textViewDidBeginEditing(_ textView: UITextView) {
            if !applyingUpdate { parent.focused = true }
        }
        func textViewDidEndEditing(_ textView: UITextView) {
            if !applyingUpdate { parent.focused = false }
        }
        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            if text == "\n", textView.markedTextRange == nil,
               (textView as? ChatTextView)?.insertingLineBreak != true {
                parent.onSend()
                return false
            }
            parent.onManualChange()
            return true
        }
    }
}

final class ChatTextView: UITextView {
    var insertingLineBreak = false
    var onEscape: (() -> Void)?
    var onInteraction: (() -> Void)?
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        onInteraction?()
        super.touchesBegan(touches, with: event)
    }
    override var keyCommands: [UIKeyCommand]? {
        let newline = UIKeyCommand(input: "\r", modifierFlags: .shift, action: #selector(insertLineBreak))
        newline.discoverabilityTitle = "New line"
        newline.wantsPriorityOverSystemBehavior = true
        let escape = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(stopDictating))
        escape.discoverabilityTitle = "Stop dictating"
        escape.wantsPriorityOverSystemBehavior = true
        return (super.keyCommands ?? []) + [newline, escape]
    }
    @objc private func insertLineBreak() {
        insertingLineBreak = true
        defer { insertingLineBreak = false }
        insertText("\n")
    }
    @objc func dismissKeyboard() { resignFirstResponder() }
    @objc private func stopDictating() { onEscape?(); resignFirstResponder() }
}
