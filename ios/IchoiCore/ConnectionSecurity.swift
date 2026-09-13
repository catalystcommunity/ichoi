import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
#if canImport(Security)
import Security
#endif
#if canImport(CryptoKit)
import CryptoKit
#endif

public enum ConnectionSecurityError: Error, Equatable {
    case insecureRedirect
    case certificatePinMismatch
    case invalidRedirect
}

/// URLSession must be created with this delegate for every server request.
/// It rejects HTTPS-to-HTTP redirects and checks the configured core key pins.
public final class PinnedHTTPSDelegate: NSObject, URLSessionDelegate, URLSessionTaskDelegate, @unchecked Sendable {
    private let fingerprints: Set<String>
    private let originalScheme: String
    public init(fingerprints: [String], originalURL: URL) {
        self.fingerprints = Set(fingerprints.map(Self.normalize))
        self.originalScheme = originalURL.scheme?.lowercased() ?? "https"
    }

    public func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge, completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        #if canImport(Security)
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              let trust = challenge.protectionSpace.serverTrust,
              SecTrustEvaluateWithError(trust, nil) else {
            completionHandler(.cancelAuthenticationChallenge, nil); return
        }
        if fingerprints.isEmpty { completionHandler(.useCredential, URLCredential(trust: trust)); return }
        let matches = (0..<SecTrustGetCertificateCount(trust)).contains { index in
            guard let certificate = SecTrustGetCertificateAtIndex(trust, index) else { return false }
            return !fingerprints.isDisjoint(with: Self.fingerprintCandidates(certificate))
        }
        completionHandler(matches ? .useCredential : .cancelAuthenticationChallenge, matches ? URLCredential(trust: trust) : nil)
        #else
        completionHandler(.performDefaultHandling, nil)
        #endif
    }

    public func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        guard let destination = request.url, let scheme = destination.scheme?.lowercased() else { completionHandler(nil); return }
        if (originalScheme == "https" && scheme != "https") || (scheme == "http" && !ServerURLPolicy.isPrivateAddress(destination.host ?? "")) { completionHandler(nil) } else { completionHandler(request) }
    }

    private static func normalize(_ value: String) -> String {
        let value = value.lowercased().hasPrefix("sha256:") ? String(value.dropFirst(7)) : value
        return value.replacingOccurrences(of: ":", with: "").replacingOccurrences(of: " ", with: "").lowercased()
    }
    #if canImport(Security) && canImport(CryptoKit)
    private static func fingerprintCandidates(_ certificate: SecCertificate) -> Set<String> {
        let certificateDER = SecCertificateCopyData(certificate) as Data
        var values = [hexSHA256(certificateDER)]
        if let subjectPublicKeyInfo = subjectPublicKeyInfoDER(certificateDER) {
            values.append(hexSHA256(subjectPublicKeyInfo))
        }
        return Set(values)
    }
    private static func hexSHA256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
    #elseif canImport(Security)
    private static func fingerprintCandidates(_ certificate: SecCertificate) -> Set<String> {
        []
    }
    #endif

    #if canImport(Security)
    private struct DERElement {
        let tag: UInt8
        let fullRange: Range<Int>
        let contentRange: Range<Int>
    }

    private static func element(_ bytes: [UInt8], at offset: Int) -> DERElement? {
        guard offset >= 0, offset + 2 <= bytes.count else { return nil }
        let tag = bytes[offset]
        let firstLength = bytes[offset + 1]
        var cursor = offset + 2
        let length: Int
        if firstLength & 0x80 == 0 {
            length = Int(firstLength)
        } else {
            let lengthBytes = Int(firstLength & 0x7f)
            guard lengthBytes > 0, lengthBytes <= 4, cursor + lengthBytes <= bytes.count else { return nil }
            var value = 0
            for byte in bytes[cursor..<(cursor + lengthBytes)] {
                value = (value << 8) | Int(byte)
            }
            cursor += lengthBytes
            length = value
        }
        guard cursor + length <= bytes.count else { return nil }
        return DERElement(
            tag: tag,
            fullRange: offset..<(cursor + length),
            contentRange: cursor..<(cursor + length)
        )
    }

    /// Return the DER SubjectPublicKeyInfo that the Ichoi server hashes.
    private static func subjectPublicKeyInfoDER(_ certificate: Data) -> Data? {
        let bytes = [UInt8](certificate)
        guard let root = element(bytes, at: 0), root.tag == 0x30,
              let tbs = element(bytes, at: root.contentRange.lowerBound), tbs.tag == 0x30 else { return nil }
        var offset = tbs.contentRange.lowerBound
        if let version = element(bytes, at: offset), version.tag == 0xa0 { offset = version.fullRange.upperBound }
        for _ in 0..<5 {
            guard let field = element(bytes, at: offset) else { return nil }
            offset = field.fullRange.upperBound
        }
        guard let subjectPublicKeyInfo = element(bytes, at: offset), subjectPublicKeyInfo.tag == 0x30 else { return nil }
        return Data(bytes[subjectPublicKeyInfo.fullRange])
    }
    #endif
}

public enum ConnectionFactory {
    public static func makeSession(for profile: ServerProfile) throws -> URLSession {
        _ = try ServerURLPolicy.validate(url: profile.url, privateHTTPConsent: profile.allowsPrivateHTTP)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = true
        return URLSession(configuration: configuration, delegate: PinnedHTTPSDelegate(fingerprints: profile.coreKeyFingerprints, originalURL: profile.url), delegateQueue: nil)
    }
}
