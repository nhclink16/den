import DenAPI
import SwiftUI

struct RootView: View {
    @Bindable var store: AppStore
    @State private var tab = 0
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        Group {
            if store.user == nil {
                LoginView(store: store)
            } else {
                TabView(selection: $tab) {
                    Tab("Rooms", systemImage: "number", value: 0) { RoomsView(store: store) }
                    Tab("Inbox", systemImage: "tray", value: 1) {
                        NavigationStack { InboxView(store: store) { tab = 0 } }
                    }
                    .badge(store.readStates.reduce(0) { $0 + Int($1.unreadCount) })
                    Tab("Search", systemImage: "magnifyingglass", value: 2) {
                        NavigationStack { SearchView(store: store) { tab = 0 } }
                    }
                    Tab("Settings", systemImage: "gearshape", value: 3) {
                        NavigationStack { SettingsView(store: store) }
                    }
                }
                .onChange(of: store.selectedChannelId) { _, value in if value != nil { tab = 0 } }
                .onChange(of: store.targetMessageId) { _, value in if value != nil { tab = 0 } }
                .onChange(of: store.selectedThread) { _, value in if value != nil { tab = 0 } }
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if let calls = store.calls, store.user != nil {
                CallDock(session: calls.session, theme: theme,
                    requestMute: { muted in Task { do { try await calls.requestMute(muted) } catch { store.report(error) } } },
                    requestEnd: { Task { do { try await calls.requestEnd() } catch { store.report(error) } } })
            }
        }
        .onChange(of: store.calls?.session.error) { _, error in
            if let error, store.calls?.session.isActive == false { store.error = error }
        }
        .font(theme.bodyFont()).foregroundStyle(theme.ink).tint(theme.accent)
        .background(theme.bg).preferredColorScheme(store.theme.preferredColorScheme)
        .alert("Couldn't finish", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) {
            Button("OK", role: .cancel) { store.error = nil }
        } message: { Text(store.error ?? "Please try again.") }
    }
}

private struct LoginView: View {
    let store: AppStore
    @State private var origin = "https://denchat.app"
    @State private var username = ""
    @State private var password = ""
    @State private var signingIn = false
    @FocusState private var field: Field?
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private enum Field { case origin, username, password }

    init(store: AppStore) {
        self.store = store
        #if DEBUG && targetEnvironment(simulator)
        if let fixture = DebugFixture.credentials {
            _origin = State(initialValue: fixture.origin)
            _username = State(initialValue: fixture.username)
            _password = State(initialValue: fixture.password)
        }
        #endif
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 28) {
                    VStack(alignment: .leading, spacing: 8) {
                        Image("LaunchMark").resizable().scaledToFit().frame(width: 64, height: 64)
                            .accessibilityHidden(true)
                        Text("Come on in.").font(theme.bodyFont(.largeTitle).weight(.semibold))
                        Text("Your people, in one place.").foregroundStyle(theme.ink2)
                    }
                    VStack(alignment: .leading, spacing: 18) {
                        input("Server") {
                            TextField("https://denchat.app", text: $origin)
                                .keyboardType(.URL).textContentType(.URL).autocorrectionDisabled()
                                .textInputAutocapitalization(.never).focused($field, equals: .origin)
                                .submitLabel(.next).onSubmit { field = .username }
                                .accessibilityLabel("Server").accessibilityIdentifier("login-server")
                        }
                        input("Username") {
                            TextField("Username", text: $username).textContentType(.username)
                                .autocorrectionDisabled().textInputAutocapitalization(.never)
                                .focused($field, equals: .username).submitLabel(.next)
                                .onSubmit { field = .password }.accessibilityIdentifier("login-username")
                        }
                        input("Password") {
                            SecureField("Password", text: $password).textContentType(.password)
                                .focused($field, equals: .password).submitLabel(.go)
                                .onSubmit(signIn).accessibilityIdentifier("login-password")
                        }
                        Button(action: signIn) {
                            HStack {
                                Spacer()
                                if signingIn { ProgressView().tint(theme.bg) }
                                Text(signingIn ? "Signing in…" : "Sign in").fontWeight(.semibold)
                                Spacer()
                            }.frame(minHeight: 44)
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(signingIn || username.trimmingCharacters(in: .whitespaces).isEmpty || password.isEmpty)
                        .accessibilityIdentifier("login-submit")
                    }
                    Text("Use the account you already have on your Den server.")
                        .font(theme.bodyFont(.footnote)).foregroundStyle(theme.ink3)
                }
                .padding(28).frame(maxWidth: 460).frame(maxWidth: .infinity)
            }
            .scrollDismissesKeyboard(.interactively).background(theme.bg)
        }
    }

    private func input<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(label).font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2)
            content().padding(13).background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius))
        }
    }

    private func signIn() {
        guard !signingIn, !username.isEmpty, !password.isEmpty else { return }
        field = nil
        signingIn = true
        Task {
            defer { signingIn = false }
            do { try await store.login(origin: origin, username: username, password: password); password = "" }
            catch { store.report(error) }
        }
    }
}
