import SwiftUI

struct LLMView: View {
    @EnvironmentObject var ollamaManager: OllamaManager
    @EnvironmentObject var dataManager: DatabaseManager
    
    @State private var selectedFile: FileRecord?
    @State private var analysisResult: String = ""
    @State private var isAnalyzing: Bool = false
    
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Model selector
            HStack {
                Text("Model:")
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                
                Picker("", selection: $ollamaManager.selectedModel) {
                    ForEach(ollamaManager.availableModels) { model in
                        Text(model.displayName).tag(model as OllamaModel?)
                    }
                }
                .pickerStyle(MenuPickerStyle())
                .frame(width: 200)
                
                Button(action: {
                    ollamaManager.loadModels()
                }) {
                    Image(systemName: "arrow.clockwise")
                        .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                }
                .buttonStyle(PlainButtonStyle())
                
                Spacer()
                
                ConnectionStatusView()
            }
            
            Divider()
                .background(Color(red: 0.2, green: 0.2, blue: 0.25))
            
            // File selection
            HStack {
                Text("Analyze File:")
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                
                Picker("", selection: $selectedFile) {
                    Text("Select a file...").tag(nil as FileRecord?)
                    ForEach(dataManager.topFiles) { file in
                        Text((file.path as NSString).lastPathComponent).tag(file as FileRecord?)
                    }
                }
                .pickerStyle(MenuPickerStyle())
                .frame(maxWidth: .infinity)
                
                Button(action: {
                    analyzeSelectedFile()
                }) {
                    Text("Analyze")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundColor(Color.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(Color(red: 0.3, green: 0.6, blue: 0.9))
                        .cornerRadius(4)
                }
                .buttonStyle(PlainButtonStyle())
                .disabled(selectedFile == nil || ollamaManager.isGenerating)
            }
            
            // Analysis result
            if isAnalyzing {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if !analysisResult.isEmpty {
                ScrollView {
                    Text(analysisResult)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundColor(Color.white)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(16)
                }
                .background(Color(red: 0.08, green: 0.08, blue: 0.1))
                .cornerRadius(8)
            } else {
                VStack {
                    Spacer()
                    Text("Select a file and click Analyze to get LLM-powered insights")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundColor(Color(red: 0.5, green: 0.5, blue: 0.6))
                    Spacer()
                }
                .frame(maxWidth: .infinity)
            }
        }
        .padding(16)
    }
    
    private func analyzeSelectedFile() {
        guard let file = selectedFile else { return }
        
        isAnalyzing = true
        analysisResult = ""
        
        // Read file content
        guard let content = try? String(contentsOfFile: file.path) else {
            analysisResult = "Error: Could not read file content"
            isAnalyzing = false
            return
        }
        
        ollamaManager.analyzeFile(path: file.path, content: content) { result in
            DispatchQueue.main.async {
                isAnalyzing = false
                switch result {
                case .success(let response):
                    analysisResult = response
                case .failure(let error):
                    analysisResult = "Error: \(error.localizedDescription)"
                }
            }
        }
    }
}

struct ConnectionStatusView: View {
    @EnvironmentObject var ollamaManager: OllamaManager
    
    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(ollamaManager.isConnected ? Color(red: 0.4, green: 0.8, blue: 0.5) : Color(red: 0.9, green: 0.4, blue: 0.4))
                .frame(width: 8, height: 8)
            
            Text(ollamaManager.isConnected ? "Connected" : "Disconnected")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundColor(ollamaManager.isConnected ? Color(red: 0.4, green: 0.8, blue: 0.5) : Color(red: 0.9, green: 0.4, blue: 0.4))
        }
    }
}
