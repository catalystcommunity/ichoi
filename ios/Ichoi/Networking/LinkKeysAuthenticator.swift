import AuthenticationServices
import Foundation

enum LinkKeysAuthenticationError: Error { case cancelled, invalidCallback, missingExchangeCode }

@MainActor
final class LinkKeysAuthenticator: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    func authenticate(server: ServerProfile, identity: String, presentationAnchor: ASPresentationAnchor) async throws -> String {
        let session = try ConnectionFactory.makeSession(for: server)
        let (statusData, statusResponse) = try await session.data(from: server.url.appendingPathComponent("api/auth"))
        guard let statusHTTP = statusResponse as? HTTPURLResponse,
              (200..<300).contains(statusHTTP.statusCode),
              let status = try? JSONDecoder().decode(LinkKeysStatus.self, from: statusData),
              let startPath = status.regularRp?.startURL ?? status.localRp?.startURL else {
            throw LinkKeysAuthenticationError.invalidCallback
        }
        var start = URLRequest(url: server.url.appendingPathComponent(
            startPath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        ))
        start.httpMethod = "POST"
        start.setValue("application/json", forHTTPHeaderField: "Content-Type")
        var originParts = URLComponents()
        originParts.scheme = server.url.scheme
        originParts.host = server.url.host
        originParts.port = server.url.port
        guard let origin = originParts.url?.absoluteString else {
            throw LinkKeysAuthenticationError.invalidCallback
        }
        start.setValue(origin, forHTTPHeaderField: "Origin")
        start.httpBody = try JSONEncoder().encode(LinkKeysStartRequest(
            identity: identity,
            returnURL: "ichoi://linkkeys"
        ))
        let (data, response) = try await session.data(for: start)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              let redirect = try? JSONDecoder().decode(LinkKeysStartResponse.self, from: data),
              let url = URL(string: redirect.redirectURL) else { throw LinkKeysAuthenticationError.invalidCallback }
        return try await withCheckedThrowingContinuation { continuation in
            self.session = ASWebAuthenticationSession(url: url, callbackURLScheme: "ichoi") { callback, error in
                guard error == nil, let callback else {
                    continuation.resume(throwing: LinkKeysAuthenticationError.cancelled)
                    return
                }
                let parts = URLComponents(url: callback, resolvingAgainstBaseURL: false)
                let queryCode = parts?.queryItems?.first(where: { $0.name == "code" || $0.name == "linkkeys_exchange" })?.value
                let fragmentCode = parts?.fragment?.split(separator: "&").first(where: { $0.hasPrefix("linkkeys_exchange=") }).map { String($0.dropFirst("linkkeys_exchange=".count)) }
                guard let code = queryCode ?? fragmentCode, !code.isEmpty else {
                    continuation.resume(throwing: LinkKeysAuthenticationError.missingExchangeCode); return
                }
                continuation.resume(returning: code)
            }
            self.session?.presentationContextProvider = self
            self.session?.prefersEphemeralWebBrowserSession = true
            self.session?.start()
        }
    }

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor { anchor }
    private var anchor: ASPresentationAnchor!
    func setPresentationAnchor(_ anchor: ASPresentationAnchor) { self.anchor = anchor }
}

private struct LinkKeysStartResponse: Decodable {
    let redirectURL: String
    enum CodingKeys: String, CodingKey { case redirectURL = "redirect_url" }
}

private struct LinkKeysStartRequest: Encodable {
    let identity: String
    let returnURL: String
    enum CodingKeys: String, CodingKey { case identity; case returnURL = "return_url" }
}

private struct LinkKeysStatus: Decodable {
    let localRp: LinkKeysProvider?
    let regularRp: LinkKeysProvider?
    enum CodingKeys: String, CodingKey { case localRp = "local_rp"; case regularRp = "regular_rp" }
}

private struct LinkKeysProvider: Decodable {
    let startURL: String
    enum CodingKeys: String, CodingKey { case startURL = "start_url" }
}
