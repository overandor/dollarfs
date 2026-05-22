import SwiftUI
import CoreData
import SQLite3

@main
struct DollarfsDashboardApp: App {
    @StateObject private var dataManager = DatabaseManager.shared
    @StateObject private var ollamaManager = OllamaManager.shared
    
    var body: some Scene {
        WindowGroup {
            DashboardView()
                .environmentObject(dataManager)
                .environmentObject(ollamaManager)
                .frame(minWidth: 1200, minHeight: 800)
        }
        .windowStyle(HiddenTitleBarWindowStyle())
    }
}
