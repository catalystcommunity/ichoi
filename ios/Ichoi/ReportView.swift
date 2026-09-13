import SwiftUI

struct ReportView: View {
    let targetType: ContentReportTargetType
    let targetID: String
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var reason: ContentReportReason = .objectionableContent
    @State private var details = ""
    @State private var error: String?
    var body: some View {
        NavigationStack {
            Form {
                Picker("Reason", selection: $reason) { ForEach(ContentReportReason.allCases, id: \.self) { Text($0.displayName).tag($0) } }
                TextEditor(text: $details).frame(minHeight: 100)
                Text("Optional details: \(details.unicodeScalars.count)/2000").font(.footnote)
                Button("Send report") { Task { await submit() } }.disabled(details.unicodeScalars.count > 2_000)
            }.navigationTitle("Report \(targetType == .playlist ? "playlist" : "user")")
        }.alert("Report", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK", role: .cancel) {} } message: { Text(error ?? "") }
    }
    private func submit() async {
        guard let profile = model.profile, let sessionManager = model.sessionManager, sessionManager.session != nil else { error = "Sign in before you report content."; return }
        do {
            let clean = try ComplianceValidation.details(details)
            let token = try KeychainTokenStore().token(for: profile.id)
            let transport = try CSILHTTPTransport(profile: profile, token: token)
            _ = try await LibraryAsyncClient(transport: transport).reportContent(ReportContentRequest(targetType: targetType, targetId: targetID, reason: reason, details: clean))
            dismiss()
        } catch ComplianceValidationError.detailsTooLong { error = "Details must be 2,000 Unicode characters or fewer." }
        catch { error = "The report was not sent. Contact the administrator of \(profile.url.absoluteString)." }
    }
}

private extension ContentReportReason {
    var displayName: String {
        switch self { case .objectionableContent: return "Objectionable content"; case .harassment: return "Harassment"; case .spam: return "Spam"; case .other: return "Other" }
    }
}
