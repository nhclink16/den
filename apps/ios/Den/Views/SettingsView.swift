import DenAPI
import SwiftUI
import UIKit

struct SettingsView: View {
    let store: AppStore
    @State private var showLogout = false
    @State private var loggingOut = false
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            Section("Account") {
                if let user = store.user {
                    HStack(spacing: 12) {
                        PersonAvatar(userId: user.id, store: store, size: 44)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(user.displayName.isEmpty ? user.username : user.displayName).fontWeight(.semibold)
                            Text("@" + user.username).font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2)
                        }
                    }.padding(.vertical, 6)
                }
                LabeledContent("Server", value: store.origin.host() ?? store.origin.absoluteString)
                    .font(theme.bodyFont(.subheadline))
            }.listRowBackground(theme.bg2)
            Section {
                NavigationLink { AppearanceView(store: store) } label: { Label("Appearance", systemImage: "paintpalette") }
                NavigationLink { NotificationSettingsView(store: store) } label: { Label("Notifications", systemImage: "bell") }
                NavigationLink { VoiceSettingsView(store: store) } label: { Label("Voice", systemImage: "waveform") }
                    .accessibilityIdentifier("settings-voice")
            }.listRowBackground(theme.bg2)
            Section {
                NavigationLink { MachinesView(store: store) } label: { Label("Machines", systemImage: "desktopcomputer") }
                NavigationLink { AccessView(store: store) } label: { Label("Access", systemImage: "key") }
            }.listRowBackground(theme.bg2)
            Section {
                Button(loggingOut ? "Signing out…" : "Sign out", role: .destructive) { showLogout = true }
                    .disabled(loggingOut).accessibilityIdentifier("account-logout")
            } footer: {
                Text("Den \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0")")
            }.listRowBackground(theme.bg2)
        }
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle("Settings")
        .confirmationDialog("Sign out of Den?", isPresented: $showLogout, titleVisibility: .visible) {
            Button("Sign out", role: .destructive) {
                loggingOut = true
                Task {
                    defer { loggingOut = false }
                    do { try await store.logout() } catch { store.report(error) }
                }
            }
        } message: { Text("You can sign back in with your account. This removes saved messages from this iPhone.") }
    }
}

private struct VoiceSettingsView: View {
    let store: AppStore
    @State private var checking = true
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            if let dictation = store.dictation {
                @Bindable var dictation = dictation
                Section {
                    Picker("Language", selection: $dictation.selectedLanguageID) {
                        if !dictation.languages.contains(where: { $0.id == dictation.selectedLanguageID }) {
                            Text(Locale.current.localizedString(forIdentifier: dictation.selectedLanguageID) ?? dictation.selectedLanguageID)
                                .tag(dictation.selectedLanguageID)
                        }
                        ForEach(dictation.languages) { language in
                            Text(language.name).tag(language.id)
                        }
                    }.accessibilityIdentifier("dictation-language")
                    if dictation.supportsPunctuation {
                        Toggle("Punctuation", isOn: $dictation.punctuationEnabled)
                            .accessibilityIdentifier("dictation-punctuation")
                    }
                    if checking { ProgressView("Checking available languages…") }
                    else if !dictation.languages.contains(where: { $0.id == dictation.selectedLanguageID }) {
                        Text("On-device dictation isn't available for this language.")
                            .foregroundStyle(theme.ink2)
                    }
                } header: { Text("Dictation") } footer: {
                    Text("Audio stays on this device. Dictation is unavailable during calls.")
                }.listRowBackground(theme.bg2)
            }
        }
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle("Voice").navigationBarTitleDisplayMode(.inline)
        .task {
            await store.dictation?.refresh(force: true)
            checking = false
        }
    }
}

private struct AppearanceView: View {
    let store: AppStore
    @State private var saving = false
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Mode").font(theme.bodyFont(.headline))
                    Picker("Mode", selection: Binding(get: { store.theme.appearance.mode }, set: { mode in
                        var appearance = store.theme.appearance
                        appearance.mode = mode
                        save(appearance)
                    })) {
                        Text("System").tag(API.AppearanceMode.system)
                        Text("Light").tag(API.AppearanceMode.light)
                        Text("Dark").tag(API.AppearanceMode.dark)
                    }.pickerStyle(.segmented).disabled(saving).accessibilityIdentifier("appearance-mode")
                    Text("Your theme follows you across Den.").font(theme.bodyFont(.footnote)).foregroundStyle(theme.ink2)
                }
                Text((store.theme.preferredColorScheme ?? colorScheme) == .dark ? "Dark theme" : "Light theme")
                    .font(theme.bodyFont(.headline))
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 155), spacing: 12)], spacing: 16) {
                    ForEach(store.theme.allFamilies, id: \.id) { family in
                        ThemeCard(family: family, selected: family.id == store.theme.selectedFamily(for: colorScheme).id,
                                  mode: store.theme.appearance.mode,
                                  custom: !store.theme.builtins.contains { $0.id == family.id }) {
                            var appearance = store.theme.appearance
                            if (store.theme.preferredColorScheme ?? colorScheme) == .dark {
                                appearance.darkTheme = family.id
                            } else { appearance.lightTheme = family.id }
                            save(appearance)
                        }.disabled(saving)
                    }
                }
                if saving { ProgressView("Saving appearance…").font(theme.bodyFont(.footnote)) }
            }.padding(20)
        }
        .background(theme.bg).navigationTitle("Appearance").navigationBarTitleDisplayMode(.inline)
    }

    private func save(_ appearance: API.Appearance) {
        guard !saving else { return }
        saving = true
        Task {
            defer { saving = false }
            do { try await store.saveAppearance(appearance) } catch { store.report(error) }
        }
    }
}

private struct ThemeCard: View {
    let family: API.Theme
    let selected: Bool
    let mode: API.AppearanceMode
    let custom: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { DenTheme(family: family, scheme: colorScheme) }

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 9) {
                ZStack {
                    half(.light).opacity(mode == .dark ? 0.7 : 1).clipShape(PreviewHalf(upper: true))
                    half(.dark).opacity(mode == .light ? 0.7 : 1).clipShape(PreviewHalf(upper: false))
                    if selected {
                        Image(systemName: "checkmark.circle.fill").font(.title3)
                            .foregroundStyle(theme.accent, theme.bg)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing).padding(10)
                    }
                }
                .frame(height: 100)
                .clipShape(RoundedRectangle(cornerRadius: theme.radius))
                .overlay(RoundedRectangle(cornerRadius: theme.radius).stroke(selected ? theme.accent : theme.line, lineWidth: selected ? 2 : 1))
                Text(family.name).font(theme.displayFont(.headline)).foregroundStyle(theme.ink)
                if custom { Text("Custom").font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2) }
            }
            .padding(10).background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius + 10))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(family.name)\(custom ? ", custom theme" : "")")
        .accessibilityAddTraits(selected ? [.isSelected] : [])
        .accessibilityIdentifier("theme-\(family.id)")
    }

    private func half(_ scheme: ColorScheme) -> some View {
        let palette = DenTheme(family: family, scheme: scheme)
        return HStack(alignment: .top, spacing: 10) {
            RoundedRectangle(cornerRadius: palette.smallRadius).fill(palette.bg3).frame(width: 22)
            VStack(alignment: .leading, spacing: 9) {
                Capsule().fill(palette.accent).frame(width: 35, height: 5)
                Capsule().fill(palette.ink).frame(height: 4)
                Capsule().fill(palette.ink2).frame(height: 4)
                Capsule().fill(palette.ink3).frame(width: 25, height: 4)
                Spacer(minLength: 0)
            }.padding(.vertical, 6)
        }.padding(12).frame(maxWidth: .infinity, maxHeight: .infinity).background(palette.bg)
    }
}

private struct PreviewHalf: Shape {
    let upper: Bool
    func path(in rect: CGRect) -> Path {
        Path { path in
            path.move(to: CGPoint(x: rect.minX, y: rect.maxY))
            path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
            if upper { path.addLine(to: CGPoint(x: rect.minX, y: rect.minY)) }
            else { path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY)) }
            path.closeSubpath()
        }
    }
}

private struct NotificationSettingsView: View {
    let store: AppStore
    @State private var saving = false
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            if let status = store.notifications?.status {
                Section { Text(status).font(theme.bodyFont(.footnote)).foregroundStyle(theme.ink2) }
                    .listRowBackground(theme.bg2)
            }
            Section {
                Toggle("Mentions", isOn: preference(\.mentions))
                Toggle("Direct messages", isOn: preference(\.dms))
            } footer: { Text("Choose which messages should notify you. iPhone notification permission is managed in Settings.") }
                .listRowBackground(theme.bg2)
            Section("Followed rooms") {
                ForEach(store.channels.filter { $0.kind == .text }, id: \.id) { channel in
                    Toggle(store.channelTitle(channel), isOn: Binding(get: {
                        store.preferences.subscribedChannelIds.contains(channel.id)
                    }, set: { enabled in
                        var value = store.preferences
                        value.subscribedChannelIds.removeAll { $0 == channel.id }
                        if enabled { value.subscribedChannelIds.append(channel.id) }
                        save(value)
                    }))
                }
            }.listRowBackground(theme.bg2)
            Section {
                Link("Open iPhone notification settings", destination: URL(string: UIApplication.openNotificationSettingsURLString)!)
            }.listRowBackground(theme.bg2)
        }
        .disabled(saving).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle("Notifications").navigationBarTitleDisplayMode(.inline)
    }

    private func preference(_ key: WritableKeyPath<API.NotificationPreferences, Bool>) -> Binding<Bool> {
        Binding(get: { store.preferences[keyPath: key] }, set: { enabled in
            var value = store.preferences
            value[keyPath: key] = enabled
            save(value)
        })
    }

    private func save(_ value: API.NotificationPreferences) {
        guard !saving else { return }
        saving = true
        Task {
            defer { saving = false }
            do { try await store.savePreferences(value) } catch { store.report(error) }
        }
    }
}

private struct MachinesView: View {
    let store: AppStore
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            if store.hosts.isEmpty { ContentUnavailableView("No machines", systemImage: "desktopcomputer", description: Text("Connect machines from the desktop app.")) }
            ForEach(store.hosts, id: \.id) { host in
                HStack(spacing: 12) {
                    Image(systemName: "desktopcomputer").foregroundStyle(theme.ink2)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(host.name)
                        Text(host.online ? "Online" : "Offline").font(theme.bodyFont(.caption))
                            .foregroundStyle(host.online ? theme.accent : theme.ink3)
                    }
                    Spacer()
                }.padding(.vertical, 4).listRowBackground(theme.bg2)
            }
        }.scrollContentBackground(.hidden).background(theme.bg)
            .navigationTitle("Machines").navigationBarTitleDisplayMode(.inline)
            .refreshable { do { try await store.refresh() } catch { store.report(error) } }
    }
}

private struct AccessView: View {
    let store: AppStore
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            if store.grants.isEmpty { ContentUnavailableView("No access grants", systemImage: "key", description: Text("Manage machine access from the desktop app.")) }
            ForEach(store.grants, id: \.id) { grant in
                VStack(alignment: .leading, spacing: 6) {
                    Text(store.hosts.first(where: { $0.id == grant.hostId })?.name ?? "Machine").fontWeight(.semibold)
                    Text("\(store.userName(grant.granteeId)) · \(grant.capability == .terminalControl ? "Terminal control" : "Terminal viewing")")
                        .font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2)
                    if grant.revokedAt != nil { Text("Revoked").foregroundStyle(theme.danger) }
                    else if let expires = grant.expiresAt {
                        Text("Expires \(Date(timeIntervalSince1970: Double(expires)).formatted(date: .abbreviated, time: .shortened))")
                            .foregroundStyle(theme.ink3)
                    } else { Text("No expiry").foregroundStyle(theme.ink3) }
                }.font(theme.bodyFont(.caption)).padding(.vertical, 5).listRowBackground(theme.bg2)
            }
        }.scrollContentBackground(.hidden).background(theme.bg)
            .navigationTitle("Access").navigationBarTitleDisplayMode(.inline)
            .refreshable { do { try await store.refresh() } catch { store.report(error) } }
    }
}
