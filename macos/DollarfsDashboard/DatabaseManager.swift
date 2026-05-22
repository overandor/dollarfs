import Foundation
import SQLite3
import Combine

class DatabaseManager: ObservableObject {
    static let shared = DatabaseManager()
    
    @Published var totalFiles: Int = 0
    @Published var totalBookValue: Double = 0.0
    @Published var securityFindings: Int = 0
    @Published var topFiles: [FileRecord] = []
    @Published var dailyLedger: DailyLedger? = nil
    @Published var isLoading: Bool = false
    
    private var db: OpaquePointer?
    private let dbPath: String
    
    private init() {
        let homeDir = FileManager.default.homeDirectoryForCurrentUser
        let configDir = homeDir.appendingPathComponent(".local_file_value")
        self.dbPath = configDir.appendingPathComponent("lfv.db").path
        
        // Ensure config directory exists
        try? FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
        
        openDatabase()
        loadStats()
    }
    
    private func openDatabase() {
        if sqlite3_open(dbPath, &db) == SQLITE_OK {
            print("Database opened successfully")
        } else {
            print("Failed to open database")
        }
    }
    
    func loadStats() {
        isLoading = true
        
        let query = "SELECT COUNT(*), COALESCE(SUM(book_value_usd), 0) FROM files WHERE deleted_at IS NULL"
        var statement: OpaquePointer?
        
        if sqlite3_prepare_v2(db, query, -1, &statement, nil) == SQLITE_OK {
            if sqlite3_step(statement) == SQLITE_ROW {
                totalFiles = Int(sqlite3_column_int64(statement, 0))
                totalBookValue = sqlite3_column_double(statement, 1)
            }
        }
        sqlite3_finalize(statement)
        
        // Load security findings
        let securityQuery = "SELECT COUNT(*) FROM security_findings"
        if sqlite3_prepare_v2(db, securityQuery, -1, &statement, nil) == SQLITE_OK {
            if sqlite3_step(statement) == SQLITE_ROW {
                securityFindings = Int(sqlite3_column_int64(statement, 0))
            }
        }
        sqlite3_finalize(statement)
        
        // Load top files
        loadTopFiles()
        
        // Load daily ledger
        loadDailyLedger()
        
        isLoading = false
    }
    
    private func loadTopFiles() {
        let query = "SELECT path, book_value_usd, valuation_confidence, security_status FROM files WHERE deleted_at IS NULL ORDER BY book_value_usd DESC LIMIT 10"
        var statement: OpaquePointer?
        
        var files: [FileRecord] = []
        
        if sqlite3_prepare_v2(db, query, -1, &statement, nil) == SQLITE_OK {
            while sqlite3_step(statement) == SQLITE_ROW {
                if let cPath = sqlite3_column_text(statement, 0) {
                    let path = String(cString: cPath)
                    let confidence = sqlite3_column_text(statement, 2).map { String(cString: $0) } ?? "unknown"
                    let securityStatus = sqlite3_column_text(statement, 3).map { String(cString: $0) } ?? "unknown"
                    
                    let file = FileRecord(
                        path: path,
                        bookValue: sqlite3_column_double(statement, 1),
                        confidence: confidence,
                        securityStatus: securityStatus
                    )
                    files.append(file)
                }
            }
        }
        sqlite3_finalize(statement)
        
        topFiles = files
    }
    
    private func loadDailyLedger() {
        let today = ISO8601DateFormatter().string(from: Date())
        let query = "SELECT files_created, files_modified, gross_value_created, net_value_created FROM daily_ledgers WHERE date = ?"
        var statement: OpaquePointer?
        
        if sqlite3_prepare_v2(db, query, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, (today as NSString).utf8String, -1, nil)
            
            if sqlite3_step(statement) == SQLITE_ROW {
                dailyLedger = DailyLedger(
                    filesCreated: Int(sqlite3_column_int64(statement, 0)),
                    filesModified: Int(sqlite3_column_int64(statement, 1)),
                    grossValue: sqlite3_column_double(statement, 2),
                    netValue: sqlite3_column_double(statement, 3)
                )
            }
        }
        sqlite3_finalize(statement)
    }
    
    func refresh() {
        loadStats()
    }
}

struct FileRecord: Identifiable, Hashable {
    let id = UUID()
    let path: String
    let bookValue: Double
    let confidence: String
    let securityStatus: String
}

struct DailyLedger {
    let filesCreated: Int
    let filesModified: Int
    let grossValue: Double
    let netValue: Double
}
