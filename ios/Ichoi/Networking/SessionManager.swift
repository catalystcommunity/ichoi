import Foundation
import AuthenticationServices
import SwiftUI

@MainActor
final class SessionManager: ObservableObject {
    @Published private(set) var session: SessionInfo?
    private let tokenStore: TokenStore
    private let profile: ServerProfile
    private let authenticator: LinkKeysAuthenticator

    init(profile: ServerProfile, tokenStore: TokenStore = KeychainTokenStore(), authenticator: LinkKeysAuthenticator = LinkKeysAuthenticator()) {
        self.profile = profile; self.tokenStore = tokenStore; self.authenticator = authenticator
    }

    func restore() async {
        guard let token = try? tokenStore.token(for: profile.id), !token.isEmpty,
              let transport = try? CSILHTTPTransport(profile: profile, token: token) else { return }
        // Keep a token when the server is temporarily unavailable. A successful
        // whoami response restores the account details without exposing the token.
        session = try? await SessionAsyncClient(transport: transport).whoami(Page())
    }

    func signIn(identity: String, anchor: ASPresentationAnchor) async throws {
        authenticator.setPresentationAnchor(anchor)
        let exchangeCode = try await authenticator.authenticate(server: profile, identity: identity, presentationAnchor: anchor)
        let transport = try CSILHTTPTransport(profile: profile, token: nil)
        let info = try await SessionAsyncClient(transport: transport).authenticate(AuthRequest(linkkeysExchangeCode: exchangeCode))
        guard let token = info.token, !token.isEmpty else { throw LinkKeysAuthenticationError.invalidCallback }
        try tokenStore.save(token: token, for: profile.id); session = info
    }

    func signOut() async {
        if let token = try? tokenStore.token(for: profile.id) {
            if let transport = try? CSILHTTPTransport(profile: profile, token: token) { _ = try? await SessionAsyncClient(transport: transport).logout(Page()) }
        }
        try? tokenStore.removeToken(for: profile.id); session = nil
    }

    func deleteAccount(confirming handle: String) async throws {
        guard let current = session else { throw LinkKeysAuthenticationError.invalidCallback }
        try ComplianceValidation.accountDeletionConfirmation(handle, handle: current.handle)
        guard let token = try tokenStore.token(for: profile.id) else { throw LinkKeysAuthenticationError.invalidCallback }
        let transport = try CSILHTTPTransport(profile: profile, token: token)
        _ = try await SessionAsyncClient(transport: transport).deleteAccount(DeleteAccountRequest(confirmationHandle: handle))
        // Remove local credentials only after the server confirms deletion.
        try tokenStore.removeToken(for: profile.id); session = nil
    }
}
