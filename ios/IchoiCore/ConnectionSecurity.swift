import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public enum ConnectionSecurityError: Error, Equatable {
    case insecureRedirect
    case invalidRedirect
}

/// URLSession must be created with this delegate for every server request.
/// iOS checks the server certificate, the same as a browser. The app has no certificate pins:
/// the server sits behind a reverse proxy with a public CA certificate, and that certificate
/// rotates. This delegate only refuses an HTTPS-to-HTTP redirect and a redirect to public HTTP.
public final class ServerSessionDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    private let originalScheme: String
    public init(originalURL: URL) {
        self.originalScheme = originalURL.scheme?.lowercased() ?? "https"
    }

    public func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        guard let destination = request.url, let scheme = destination.scheme?.lowercased() else { completionHandler(nil); return }
        if (originalScheme == "https" && scheme != "https") || (scheme == "http" && !ServerURLPolicy.isPrivateAddress(destination.host ?? "")) { completionHandler(nil) } else { completionHandler(request) }
    }
}

public enum ConnectionFactory {
    public static func makeSession(for profile: ServerProfile) throws -> URLSession {
        _ = try ServerURLPolicy.validate(url: profile.url, privateHTTPConsent: profile.allowsPrivateHTTP)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = true
        return URLSession(configuration: configuration, delegate: ServerSessionDelegate(originalURL: profile.url), delegateQueue: nil)
    }
}


/// Validate byte ranges before the media loader accepts response data.
public struct MediaByteRange: Equatable {
    public let start: Int64
    public let end: Int64
    public let total: Int64
    public static let chunkSize: Int64 = 512 * 1024
    public static func chunkEnd(start: Int64, exclusiveEnd: Int64) -> Int64 {
        min(exclusiveEnd - 1, start + min(chunkSize - 1, Int64.max - start))
    }
    public static func parse(_ header: String?, requestedStart: Int64, requestedEnd: Int64) -> MediaByteRange? {
        guard let header, header.hasPrefix("bytes ") else { return nil }
        let parts = header.dropFirst(6).split(separator: "/")
        guard parts.count == 2, let total = Int64(parts[1]), total > 0 else { return nil }
        let bounds = parts[0].split(separator: "-")
        guard bounds.count == 2, let start = Int64(bounds[0]), let end = Int64(bounds[1]),
              start == requestedStart, start >= 0, end >= start, end <= requestedEnd,
              end < total, end - start < chunkSize else { return nil }
        return MediaByteRange(start: start, end: end, total: total)
    }
}
