import SwiftUI

struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var showDelete = false
    @State private var handle = ""
    @State private var identity = ""
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
                    } else { Section("Account") { Text("Sign in with LinkKeys to save progress, report content, or manage an account."); TextField("handle@domain", text: $identity).textInputAutocapitalization(.never).autocorrectionDisabled(); Button("Sign in") { Task { await model.signIn(identity: identity) } } } }
                }
            }.navigationTitle("Settings").toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
        }
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
        } message: { Text("This removes your account and private data from the selected server. Type the exact handle to continue.") }
    }
}
