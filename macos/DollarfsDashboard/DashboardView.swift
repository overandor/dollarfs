import SwiftUI

extension Notification.Name {
    static let tabChanged = Notification.Name("tabChanged")
}

struct DashboardView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        VStack(spacing: 0) {
            // Header
            HeaderView()
                .padding(.horizontal, 24)
                .padding(.vertical, 16)
                .background(Color.black)
            
            // Main content
            HStack(spacing: 0) {
                // Sidebar
                SidebarView()
                    .frame(width: 200)
                    .background(Color(red: 0.08, green: 0.08, blue: 0.1))
                
                // Content area
                ContentView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Color.black)
    }
}

struct HeaderView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        HStack {
            Text("DOLLARFS")
                .font(.system(size: 20, weight: .bold, design: .monospaced))
                .foregroundColor(Color(red: 0.4, green: 0.6, blue: 0.8))
            
            Spacer()
            
            Text("v0.2.0")
                .font(.system(size: 12, weight: .medium, design: .monospaced))
                .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
            
            Button(action: {
                dataManager.refresh()
            }) {
                Image(systemName: "arrow.clockwise")
                    .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
            }
            .buttonStyle(PlainButtonStyle())
            .padding(.leading, 16)
        }
    }
}

struct SidebarView: View {
    @State private var selectedTab: SidebarTab = .overview
    
    enum SidebarTab: String, CaseIterable {
        case overview = "Overview"
        case files = "Files"
        case security = "Security"
        case ledger = "Ledger"
        case llm = "LLM"
        case settings = "Settings"
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(SidebarTab.allCases, id: \.self) { tab in
                Button(action: {
                    selectedTab = tab
                    NotificationCenter.default.post(name: .tabChanged, object: tab)
                }) {
                    HStack {
                        Text(tab.rawValue)
                            .font(.system(size: 13, weight: .medium, design: .monospaced))
                            .foregroundColor(selectedTab == tab ? Color.white : Color(red: 0.5, green: 0.5, blue: 0.6))
                        Spacer()
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                    .background(selectedTab == tab ? Color(red: 0.15, green: 0.15, blue: 0.2) : Color.clear)
                }
                .buttonStyle(PlainButtonStyle())
            }
            
            Spacer()
        }
    }
}

struct ContentView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    @State private var selectedTab: SidebarView.SidebarTab = .overview
    
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                if dataManager.isLoading {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    switch selectedTab {
                    case .overview:
                        OverviewContent()
                    case .llm:
                        LLMView()
                    default:
                        Text("Coming soon")
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                    }
                }
            }
            .padding(24)
        }
        .background(Color(red: 0.05, green: 0.05, blue: 0.07))
        .onReceive(NotificationCenter.default.publisher(for: .tabChanged)) { notification in
            if let tab = notification.object as? SidebarView.SidebarTab {
                selectedTab = tab
            }
        }
    }
}

struct OverviewContent: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            StatsCardsView()
            TopFilesView()
            DailyLedgerView()
        }
    }
}

struct StatsCardsView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        HStack(spacing: 16) {
            StatCard(
                title: "Total Files",
                value: "\(dataManager.totalFiles)",
                color: Color(red: 0.4, green: 0.6, blue: 0.8)
            )
            
            StatCard(
                title: "Book Value",
                value: String(format: "$%.0f", dataManager.totalBookValue),
                color: Color(red: 0.3, green: 0.8, blue: 0.5)
            )
            
            StatCard(
                title: "Security Issues",
                value: "\(dataManager.securityFindings)",
                color: dataManager.securityFindings > 0 ? Color(red: 0.9, green: 0.4, blue: 0.4) : Color(red: 0.4, green: 0.8, blue: 0.5)
            )
        }
    }
}

struct StatCard: View {
    let title: String
    let value: String
    let color: Color
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
            
            Text(value)
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundColor(color)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Color(red: 0.08, green: 0.08, blue: 0.1))
        .cornerRadius(8)
    }
}

struct TopFilesView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("TOP FILES")
                .font(.system(size: 13, weight: .bold, design: .monospaced))
                .foregroundColor(Color.white)
            
            VStack(spacing: 0) {
                // Header
                HStack {
                    Text("Path")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                        .frame(maxWidth: .infinity, alignment: .leading)
                    
                    Text("Value")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                        .frame(width: 80)
                    
                    Text("Risk")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                        .frame(width: 60)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(Color(red: 0.08, green: 0.08, blue: 0.1))
                
                // Rows
                ForEach(dataManager.topFiles) { file in
                    HStack {
                        Text((file.path as NSString).lastPathComponent)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundColor(Color.white)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .lineLimit(1)
                        
                        Text(String(format: "$%.0f", file.bookValue))
                            .font(.system(size: 12, weight: .medium, design: .monospaced))
                            .foregroundColor(Color(red: 0.4, green: 0.6, blue: 0.8))
                            .frame(width: 80)
                        
                        Text(file.securityStatus == "high_risk" ? "HIGH" : "OK")
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(file.securityStatus == "high_risk" ? Color(red: 0.9, green: 0.4, blue: 0.4) : Color(red: 0.4, green: 0.8, blue: 0.5))
                            .frame(width: 60)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(Color(red: 0.05, green: 0.05, blue: 0.07))
                }
            }
            .cornerRadius(8)
        }
    }
}

struct DailyLedgerView: View {
    @EnvironmentObject var dataManager: DatabaseManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("DAILY LEDGER")
                .font(.system(size: 13, weight: .bold, design: .monospaced))
                .foregroundColor(Color.white)
            
            if let ledger = dataManager.dailyLedger {
                HStack(spacing: 24) {
                    LedgerItem(title: "Created", value: "\(ledger.filesCreated)")
                    LedgerItem(title: "Modified", value: "\(ledger.filesModified)")
                    LedgerItem(title: "Gross", value: String(format: "$%.0f", ledger.grossValue))
                    LedgerItem(title: "Net", value: String(format: "$%.0f", ledger.netValue))
                }
                .padding(16)
                .background(Color(red: 0.08, green: 0.08, blue: 0.1))
                .cornerRadius(8)
            } else {
                Text("No ledger entry for today")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                    .padding(16)
                    .frame(maxWidth: .infinity)
                    .background(Color(red: 0.08, green: 0.08, blue: 0.1))
                    .cornerRadius(8)
            }
        }
    }
}

struct LedgerItem: View {
    let title: String
    let value: String
    
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.system(size: 10, weight: .medium, design: .monospaced))
                .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
            
            Text(value)
                .font(.system(size: 16, weight: .bold, design: .monospaced))
                .foregroundColor(Color.white)
        }
    }
}
