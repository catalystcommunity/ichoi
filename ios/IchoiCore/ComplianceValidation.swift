import Foundation

public enum ComplianceValidationError: Error, Equatable {
    case emptyValue
    case detailsTooLong
    case handleMismatch
    case missingProfile
    case missingAccount
}

public enum ComplianceValidation {
    public static func details(_ value: String?) throws -> String? {
        guard let value, !value.isEmpty else { return nil }
        guard value.unicodeScalars.count <= 2_000 else { throw ComplianceValidationError.detailsTooLong }
        return value
    }

    public static func accountDeletionConfirmation(_ entered: String, handle: String) throws {
        guard !entered.isEmpty else { throw ComplianceValidationError.emptyValue }
        guard entered == handle else { throw ComplianceValidationError.handleMismatch }
    }

    public static func changeAccountBlock(
        profileID: String,
        accountID: String,
        request: (String, String) async throws -> Void,
        refresh: (String) async throws -> Void
    ) async throws {
        guard !profileID.isEmpty else { throw ComplianceValidationError.missingProfile }
        guard !accountID.isEmpty else { throw ComplianceValidationError.missingAccount }
        try await request(profileID, accountID)
        try await refresh(profileID)
    }
}
