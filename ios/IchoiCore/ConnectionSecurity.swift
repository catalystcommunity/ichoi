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
