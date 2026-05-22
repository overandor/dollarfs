import Foundation
import Combine

class OllamaManager: ObservableObject {
    static let shared = OllamaManager()
    
    @Published var isConnected: Bool = false
    @Published var availableModels: [OllamaModel] = []
    @Published var selectedModel: OllamaModel? = nil
    @Published var isGenerating: Bool = false
    
    private let baseURL = "http://localhost:11434"
    private let session = URLSession.shared
    
    private init() {
        checkConnection()
        loadModels()
    }
    
    func checkConnection() {
        let url = URL(string: "\(baseURL)/api/tags")!
        
        session.dataTask(with: url) { data, response, error in
            DispatchQueue.main.async {
                self.isConnected = (error == nil)
            }
        }.resume()
    }
    
    func loadModels() {
        let url = URL(string: "\(baseURL)/api/tags")!
        
        session.dataTask(with: url) { data, response, error in
            guard let data = data,
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let models = json["models"] as? [[String: Any]] else {
                DispatchQueue.main.async {
                    self.availableModels = []
                }
                return
            }
            
            let ollamaModels = models.compactMap { dict -> OllamaModel? in
                guard let name = dict["name"] as? String,
                      let size = dict["size"] as? Int64 else {
                    return nil
                }
                return OllamaModel(name: name, size: size)
            }
            
            DispatchQueue.main.async {
                self.availableModels = ollamaModels
                if self.selectedModel == nil, !ollamaModels.isEmpty {
                    self.selectedModel = ollamaModels.first
                }
            }
        }.resume()
    }
    
    func generate(prompt: String, completion: @escaping (Result<String, Error>) -> Void) {
        guard let model = selectedModel else {
            completion(.failure(NSError(domain: "OllamaManager", code: -1, userInfo: [NSLocalizedDescriptionKey: "No model selected"])))
            return
        }
        
        isGenerating = true
        
        let url = URL(string: "\(baseURL)/api/generate")!
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        
        let body: [String: Any] = [
            "model": model.name,
            "prompt": prompt,
            "stream": false
        ]
        
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        
        session.dataTask(with: request) { data, response, error in
            DispatchQueue.main.async {
                self.isGenerating = false
                
                if let error = error {
                    completion(.failure(error))
                    return
                }
                
                guard let data = data,
                      let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let response = json["response"] as? String else {
                    completion(.failure(NSError(domain: "OllamaManager", code: -1, userInfo: [NSLocalizedDescriptionKey: "Invalid response"])))
                    return
                }
                
                completion(.success(response))
            }
        }.resume()
    }
    
    func analyzeFile(path: String, content: String, completion: @escaping (Result<String, Error>) -> Void) {
        let prompt = """
        Analyze this file for economic value and provide a brief assessment:
        
        File path: \(path)
        
        Content:
        \(content)
        
        Please provide:
        1. A one-sentence summary of purpose
        2. Estimated complexity (low/medium/high)
        3. Production readiness (yes/no/partial)
        4. Any security concerns
        5. Suggested improvements
        
        Keep it concise and actionable.
        """
        
        generate(prompt: prompt, completion: completion)
    }
    
    func explainValuation(file: FileRecord, completion: @escaping (Result<String, Error>) -> Void) {
        let prompt = """
        Explain the valuation of this file:
        
        Path: \(file.path)
        Book value: $\(String(format: "%.2f", file.bookValue))
        Confidence: \(file.confidence)
        Security status: \(file.securityStatus)
        
        Provide a brief explanation of why this file has this valuation and what factors contribute to it.
        """
        
        generate(prompt: prompt, completion: completion)
    }
}

struct OllamaModel: Identifiable, Equatable, Hashable {
    let id = UUID()
    let name: String
    let size: Int64
    
    var displayName: String {
        name.replacingOccurrences(of: ":latest", with: "")
    }
    
    var sizeFormatted: String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useGB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: size)
    }
}
