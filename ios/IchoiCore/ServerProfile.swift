import Foundation

public struct ServerProfile: Codable, Equatable, Identifiable, Sendable {
    public let id: UUID
    public var name: String
    public var url: URL
    public var coreKeyFingerprints: [String]
    public var allowsPrivateHTTP: Bool

    public init(id: UUID = UUID(), name: String, url: URL, coreKeyFingerprints: [String] = [], allowsPrivateHTTP: Bool = false) throws {
        let normalized = try ServerURLPolicy.validate(url: url, privateHTTPConsent: allowsPrivateHTTP)
        self.id = id
        self.name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        self.url = normalized
        self.coreKeyFingerprints = coreKeyFingerprints
        self.allowsPrivateHTTP = allowsPrivateHTTP
    }
}

public enum ServerURLPolicyError: Error, Equatable {
    case missingScheme
    case unsupportedScheme
    case publicHTTPNotAllowed
    case privateHTTPConsentRequired
    case invalidHost
}

public enum ServerURLPolicy {
    public static func validate(url: URL, privateHTTPConsent: Bool) throws -> URL {
        guard let scheme = url.scheme?.lowercased() else { throw ServerURLPolicyError.missingScheme }
        guard scheme == "https" || scheme == "http" else { throw ServerURLPolicyError.unsupportedScheme }
        guard let host = url.host, !host.isEmpty else { throw ServerURLPolicyError.invalidHost }
        guard url.user == nil, url.password == nil, url.query == nil, url.fragment == nil,
              url.path.isEmpty || url.path == "/" else { throw ServerURLPolicyError.invalidHost }
        if scheme == "http" {
            guard isPrivateAddress(host) else { throw ServerURLPolicyError.publicHTTPNotAllowed }
            guard privateHTTPConsent else { throw ServerURLPolicyError.privateHTTPConsentRequired }
        }
        return url
    }

    public static func isPrivateAddress(_ host: String) -> Bool {
        let value = host.trimmingCharacters(in: CharacterSet(charactersIn: "[]")).lowercased()
        if value == "localhost" || value.hasSuffix(".localhost") || value == "::1" || value == "0:0:0:0:0:0:0:1" { return true }
        if value.hasPrefix("fe80:") || value.hasPrefix("fc") || value.hasPrefix("fd") { return true }
        let octets = value.split(separator: ".").compactMap { Int($0) }
        guard octets.count == 4, octets.allSatisfy({ (0...255).contains($0) }) else { return false }
        if octets[0] == 10 || octets[0] == 127 || (octets[0] == 169 && octets[1] == 254) { return true }
        if octets[0] == 192 && octets[1] == 168 { return true }
        return octets[0] == 172 && (16...31).contains(octets[1])
    }
}

public final class ServerProfileStore: @unchecked Sendable {
    private let defaults: UserDefaults
    private let key = "ichoi.server-profiles"
    public init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    public func profiles() -> [ServerProfile] {
        guard let data = defaults.data(forKey: key), let result = try? JSONDecoder().decode([ServerProfile].self, from: data) else { return [] }
        return result
    }

    public func save(_ profile: ServerProfile) throws {
        var values = profiles().filter { $0.id != profile.id }
        values.append(profile)
        defaults.set(try JSONEncoder().encode(values), forKey: key)
    }

    public func remove(id: UUID) throws {
        defaults.set(try JSONEncoder().encode(profiles().filter { $0.id != id }), forKey: key)
    }
}
