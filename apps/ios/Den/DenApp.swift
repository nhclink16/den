import SwiftUI

@main struct DenApp: App {
    @UIApplicationDelegateAdaptor(DenAppDelegate.self) private var delegate
    private var store: AppStore { delegate.store }
    @Environment(\.scenePhase) private var scenePhase
    var body: some Scene {
        WindowGroup {
            RootView(store: store)
                .preferredColorScheme(store.theme.preferredColorScheme)
                .task {
                    await store.restore()
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active { store.foreground() }
                }
        }
    }
}
