import SwiftUI
import UIKit

@main
struct IchoiApp: App {
    @StateObject private var model = AppModel()
    @StateObject private var playback = PlaybackManager()
    var body: some Scene { WindowGroup { ContentView().environmentObject(model).environmentObject(playback) } }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var profile: ServerProfile?
    @Published var sessionManager: SessionManager?
    @Published var hasAcceptedTerms: Bool
    @Published var message: String?
    let profiles = ServerProfileStore()

    init() {
        hasAcceptedTerms = UserDefaults.standard.bool(forKey: "ichoi.terms.accepted.v1")
        profile = profiles.profiles().first
        if let profile { sessionManager = SessionManager(profile: profile) }
    }

    func acceptTerms() { hasAcceptedTerms = true; UserDefaults.standard.set(true, forKey: "ichoi.terms.accepted.v1") }
    func restoreSession() async { await sessionManager?.restore() }
    func addProfile(name: String, urlString: String, allowPrivateHTTP: Bool, pins: [String]) {
        guard let url = URL(string: urlString) else { message = "Enter a valid server URL."; return }
        do {
            let value = try ServerProfile(name: name.isEmpty ? url.host ?? "Ichoi server" : name, url: url, coreKeyFingerprints: pins, allowsPrivateHTTP: allowPrivateHTTP)
            try profiles.save(value); profile = value; sessionManager = SessionManager(profile: value); message = nil
        } catch ServerURLPolicyError.publicHTTPNotAllowed { message = "Public HTTP is not allowed. Use HTTPS." }
        catch ServerURLPolicyError.privateHTTPConsentRequired { message = "Confirm private-network HTTP before saving this server." }
        catch { message = "The server URL is not valid." }
    }
    func removeProfile() {
        guard let profile else { return }
        try? KeychainTokenStore().removeToken(for: profile.id)
        try? profiles.remove(id: profile.id)
        self.profile = nil
        sessionManager = nil
    }

    var savedProfiles: [ServerProfile] { profiles.profiles() }

    func selectProfile(_ value: ServerProfile) {
        profile = value
        sessionManager = SessionManager(profile: value)
        Task { await sessionManager?.restore() }
    }

    func prepareNewProfile() {
        profile = nil
        sessionManager = nil
    }

    func token() -> String? {
        guard let profile else { return nil }
        return try? KeychainTokenStore().token(for: profile.id)
    }

    func signIn(identity: String) async {
        guard let manager = sessionManager else { return }
        guard !identity.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { message = "Enter your LinkKeys identity."; return }
        let anchor = UIApplication.shared.connectedScenes.compactMap { scene in
            (scene as? UIWindowScene)?.windows.first(where: { $0.isKeyWindow })
        }.first ?? UIWindow()
        do { try await manager.signIn(identity: identity, anchor: anchor) }
        catch { message = "Sign in was not completed. Check the selected server." }
    }
}
