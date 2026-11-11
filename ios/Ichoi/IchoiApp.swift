import SwiftUI
import UIKit
import Combine

@main
@MainActor
struct IchoiApp: App {
    @StateObject private var model: AppModel
    @StateObject private var playback: PlaybackManager

    init() {
        let model = AppModel()
        _model = StateObject(wrappedValue: model)
        _playback = StateObject(wrappedValue: PlaybackManager(messages: model.messages))
    }

    var body: some Scene {
        WindowGroup { ContentView().environmentObject(model).environmentObject(playback).environmentObject(model.messages) }
    }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var profile: ServerProfile?
    @Published var sessionManager: SessionManager? {
        didSet { observeSession() }
    }
    @Published var hasAcceptedTerms: Bool
    let messages = AppMessages()
    var message: String? {
        get { messages.current?.text }
        set { if let newValue { messages.show(newValue) } else { messages.dismiss() } }
    }
    let profiles = ServerProfileStore()
    private var sessionChanges: AnyCancellable?

    init() {
        hasAcceptedTerms = UserDefaults.standard.bool(forKey: "ichoi.terms.accepted.v1")
        profile = profiles.profiles().first
        if let profile { sessionManager = SessionManager(profile: profile) }
        observeSession()
    }

    private func observeSession() {
        sessionChanges = sessionManager?.objectWillChange.sink { [weak self] _ in
            self?.objectWillChange.send()
        }
    }

    func acceptTerms() { hasAcceptedTerms = true; UserDefaults.standard.set(true, forKey: "ichoi.terms.accepted.v1") }
    func restoreSession() async { await sessionManager?.restore() }
    func connectDemo() {
        if let saved = savedProfiles.first(where: { $0.url == IchoiConfig.demoURL }) { selectProfile(saved); return }
        addProfile(name: "Ichoi demo", urlString: IchoiConfig.demoURL.absoluteString, allowPrivateHTTP: false)
    }
    func addProfile(name: String, urlString: String, allowPrivateHTTP: Bool) {
        guard let url = try? ServerURLPolicy.parse(urlString, privateHTTPConsent: allowPrivateHTTP) else { message = "Enter a valid server URL."; return }
        do {
            let value = try ServerProfile(name: name.isEmpty ? url.host ?? "Ichoi server" : name, url: url, allowsPrivateHTTP: allowPrivateHTTP)
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

    func signIn(identity: String) async -> Bool {
        guard let manager = sessionManager else { return false }
        guard !identity.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { message = "Enter your LinkKeys identity."; return false }
        let anchor = UIApplication.shared.connectedScenes.compactMap { scene in
            (scene as? UIWindowScene)?.windows.first(where: { $0.isKeyWindow })
        }.first ?? UIWindow()
        do {
            try await manager.signIn(identity: identity, anchor: anchor)
            return sessionManager === manager
        }
        catch {
            if sessionManager === manager { message = "Sign in was not completed. Check the selected server." }
            return false
        }
    }
}
