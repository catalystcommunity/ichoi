import SwiftUI

struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var showDelete = false
    @State private var handle = ""
    @State private var identity = ""
    @State private var blockedAccounts: [BlockedAccount] = []
    @State private var blockedAccountError: String?
    var body: some View {
        NavigationStack {
            Form {
                if let profile = model.profile {
                    Section("Selected server") {
                        Text(profile.url.absoluteString)
                        ForEach(model.savedProfiles) { saved in
                            Button {
                                model.selectProfile(saved)
                                dismiss()
                            } label: {
                                HStack {
                                    Text(saved.name)
                                    Spacer()
                                    if saved.id == profile.id { Image(systemName: "checkmark") }
                                }
                            }
                        }
                        Button("Add another server") { model.prepareNewProfile(); dismiss() }
                        Button("Remove selected server", role: .destructive) { model.removeProfile(); dismiss() }
                    }
                    Section("Legal") { Link("Privacy policy", destination: IchoiConfig.privacyURL); Link("Terms", destination: IchoiConfig.termsURL) }
                    if let manager = model.sessionManager, let session = manager.session {
                        Section("Account") {
                            Text(session.handle)
                            Button("Sign out") { Task { await manager.signOut(); dismiss() } }
                            Button("Delete account", role: .destructive) { handle = ""; showDelete = true }
                        }
                        Section("Blocked accounts") {
                            Text("A block hides public playlists from that account on this selected server.")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            if blockedAccounts.isEmpty {
                                Text("You have not blocked an account on this server.")
                                    .foregroundStyle(.secondary)
                            }
                            ForEach(blockedAccounts, id: \.accountId) { account in
                                HStack {
                                    VStack(alignment: .leading) {
                                        Text(account.displayName ?? account.handle)
                                        Text(account.accountId).font(.caption).foregroundStyle(.secondary)
                                    }
                                    Spacer()
                                    Button("Unblock") { Task { await unblock(account.accountId) } }
                                }
                            }
                            if let blockedAccountError { Text(blockedAccountError).foregroundStyle(.red) }
                        }
                    } else { Section("Account") { Text("Sign in with LinkKeys to save progress, report content, or manage an account."); TextField("handle@domain", text: $identity).textInputAutocapitalization(.never).autocorrectionDisabled(); Button("Sign in") { Task { await model.signIn(identity: identity) } } } }
                }
            }.navigationTitle("Settings").toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
        }
        .task(id: model.profile?.id) { await loadBlockedAccounts() }
        .alert("Delete account", isPresented: $showDelete) {
            TextField("Type your exact handle", text: $handle)
            Button("Delete permanently", role: .destructive) {
                guard let manager = model.sessionManager else { return }
                Task {
                    do { try await manager.deleteAccount(confirming: handle); dismiss() }
                    catch { model.message = "Deletion failed. Your session is kept. Contact the administrator of \(model.profile?.url.absoluteString ?? "the selected server")." }
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: { Text("This removes your account and every public and private playlist that you own from the selected server. Type the exact handle to continue.") }
    }

    private func libraryClient() throws -> LibraryAsyncClient {
        guard let profile = model.profile else { throw ServerURLPolicyError.invalidHost }
        return LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
    }

    private func loadBlockedAccounts() async {
        guard model.sessionManager?.session != nil, let profileID = model.profile?.id else {
            blockedAccounts = []
            return
        }
        do {
            let api = try libraryClient()
            var accounts: [BlockedAccount] = []
            while true {
                let page = try await api.listBlockedAccounts(Page(offset: UInt64(accounts.count), limit: 1_000))
                accounts += page.accounts
                if page.accounts.isEmpty || UInt64(accounts.count) >= page.total { break }
            }
            guard model.profile?.id == profileID else { return }
            blockedAccounts = accounts
            blockedAccountError = nil
        } catch {
            guard model.profile?.id == profileID else { return }
            blockedAccountError = "The blocked-account list could not be loaded from this server."
        }
    }

    private func unblock(_ accountID: String) async {
        guard let profileID = model.profile?.id else { return }
        do {
            try await ComplianceValidation.changeAccountBlock(
                profileID: profileID.uuidString,
                accountID: accountID,
                request: { _, target in
                    _ = try await libraryClient().unblockAccount(BlockAccountRequest(accountId: target))
                },
                refresh: { selectedProfileID in
                    guard model.profile?.id.uuidString == selectedProfileID else { return }
                    await loadBlockedAccounts()
                }
            )
        } catch {
            guard model.profile?.id == profileID else { return }
            blockedAccountError = "The account was not unblocked on the selected server."
        }
    }
}
