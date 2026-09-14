import SwiftUI

@main struct DenApp: App {
    @UIApplicationDelegateAdaptor(DenAppDelegate.self) private var delegate
    @State private var store = AppStore()
    @Environment(\.scenePhase) private var scenePhase
    var body: some Scene {
        WindowGroup {
            RootView(store: store)
                .preferredColorScheme(store.theme.preferredColorScheme)
                .task {
                    let notifications = NotificationController(store: store)
                    store.notifications = notifications
                    delegate.notifications = notifications
                    await store.restore()
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active { store.foreground() }
                }
        }
    }
}
