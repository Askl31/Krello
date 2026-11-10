// Krello Apple Intelligence helper.
// Protocol: prints `{"status": "available" | "unavailable:<reason>"}` on start,
// then answers one JSON request per stdin line with one JSON response line.

import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

struct Request: Codable {
    let id: Int
    let system: String?
    let prompt: String
    let maxTokens: Int?
}

struct Response: Codable {
    let id: Int
    let ok: Bool
    let text: String?
    let error: String?
}

func availabilityString() -> String {
    #if canImport(FoundationModels)
    if #available(macOS 15.1, *) {
        switch SystemLanguageModel.default.availability {
        case .available:
            return "available"
        case .unavailable(let reason):
            switch reason {
            case .deviceNotEligible:
                return "unavailable:device_not_eligible"
            case .appleIntelligenceNotEnabled:
                return "unavailable:apple_intelligence_not_enabled"
            case .modelNotReady:
                return "unavailable:model_not_ready"
            @unknown default:
                return "unavailable:unknown"
            }
        @unknown default:
            return "unavailable:unknown"
        }
    }
    return "unavailable:os_too_old"
    #else
    return "unavailable:framework_not_importable"
    #endif
}

func generate(_ req: Request) async -> Response {
    #if canImport(FoundationModels)
    if #available(macOS 15.1, *) {
        do {
            let instructions = req.system ?? "Tu es un assistant de productivité. Réponds en français, de façon concise."
            let session = LanguageModelSession(instructions: instructions)
            var options = GenerationOptions()
            if let maxTokens = req.maxTokens {
                options.maximumResponseTokens = maxTokens
            }
            let response = try await session.respond(to: req.prompt, options: options)
            return Response(id: req.id, ok: true, text: response.content, error: nil)
        } catch {
            return Response(id: req.id, ok: false, text: nil, error: "\(error)")
        }
    }
    #endif
    return Response(id: req.id, ok: false, text: nil, error: "unavailable")
}

func emit(_ line: String) {
    print(line)
    fflush(stdout)
}

emit("{\"status\": \"\(availabilityString())\"}")

while let line = readLine() {
    let trimmed = line.trimmingCharacters(in: .whitespaces)
    if trimmed.isEmpty { continue }
    guard let data = trimmed.data(using: .utf8),
          let req = try? JSONDecoder().decode(Request.self, from: data) else {
        emit("{\"id\":-1,\"ok\":false,\"text\":null,\"error\":\"bad_request\"}")
        continue
    }
    let resp = await generate(req)
    if let d = try? JSONEncoder().encode(resp),
       let s = String(data: d, encoding: .utf8) {
        emit(s)
    } else {
        emit("{\"id\":\(req.id),\"ok\":false,\"text\":null,\"error\":\"encode_failed\"}")
    }
}
