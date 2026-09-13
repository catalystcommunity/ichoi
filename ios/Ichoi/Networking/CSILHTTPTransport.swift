import Foundation

/// Handwritten WebSocket transport wrapper. The server exposes CSIL at `/ws`.
/// CSIL types and service clients are generated in server/generated/swift-client
/// and are referenced by the Xcode project.
final class CSILHTTPTransport: NSObject, AsyncCsilTransport {
    private let endpoint: URL
    private let token: String?
    private let session: URLSession

    init(profile: ServerProfile, token: String?) throws {
        var components = URLComponents(url: profile.url.appendingPathComponent("ws"), resolvingAgainstBaseURL: false)
        components?.scheme = profile.url.scheme?.lowercased() == "https" ? "wss" : "ws"
        guard let endpoint = components?.url else { throw ServerURLPolicyError.invalidHost }
        self.endpoint = endpoint
        self.token = token
        self.session = try ConnectionFactory.makeSession(for: profile)
    }

    func call(service: String, op: String, request: [UInt8]) async throws -> [UInt8] {
        let socket = session.webSocketTask(with: endpoint)
        socket.resume()
        var helloFields: [(CsilCborValue, CsilCborValue)] = [(.text("versions"), .array([.uint(1)])), (.text("profiles"), .array([.text("verbose")]))]
        if let token { helloFields.append((.text("auth"), .text(token))) }
        let helloPayload: CsilCborValue = .map(helloFields)
        try await socket.send(.data(Data(Self.envelope(event: "$hello", payload: CsilCbor.encode(helloPayload)))))
        _ = try await Self.receive(event: "$hello-ack", id: nil, socket: socket)
        let id: UInt64 = 1
        let frame = Self.envelope(service: service, event: op, id: id, payload: request)
        try await socket.send(.data(Data(frame)))
        let response = try await Self.receive(event: op, id: id, socket: socket)
        socket.cancel(with: .goingAway, reason: nil)
        return response
    }

    private static func envelope(service: String? = nil, event: String, id: UInt64? = nil, payload: [UInt8]) -> [UInt8] {
        var fields: [(CsilCborValue, CsilCborValue)] = [(.text("event"), .text(event)), (.text("payload"), .tag(24, .bytes(payload)))]
        if let service { fields.append((.text("service"), .text(service))) }
        if let id { fields.append((.text("id"), .uint(id))) }
        return CsilCbor.encode(.map(fields))
    }

    private static func receive(event: String, id: UInt64?, socket: URLSessionWebSocketTask) async throws -> [UInt8] {
        while true {
            let message = try await socket.receive()
            guard case .data(let data) = message, let value = try? CsilCbor.decode(Array(data)), case .map(let fields) = value else { continue }
            var foundEvent: String?; var foundID: UInt64?; var foundPayload: [UInt8]?
            for (key, value) in fields {
                guard case .text(let key) = key else { continue }
                if key == "event", case .text(let value) = value { foundEvent = value }
                if key == "id", case .uint(let value) = value { foundID = value }
                if key == "payload", case .tag(_, .bytes(let value)) = value { foundPayload = value }
            }
            guard foundEvent == event || (event == "$hello-ack" && foundEvent == "$hello-ack"), foundID == id || id == nil, let foundPayload else { continue }
            if let payloadValue = try? CsilCbor.decode(foundPayload), case .map(let errorFields) = payloadValue {
                var code: Int64?; var message: String?
                for (key, value) in errorFields {
                    guard case .text(let key) = key else { continue }
                    if key == "code", case .int(let value) = value { code = value }
                    if key == "code", case .uint(let value) = value { code = Int64(value) }
                    if key == "message", case .text(let value) = value { message = value }
                }
                if let code, let message { throw CsilClientError(code: code, message: message) }
            }
            return foundPayload
        }
    }
}
