import XCTest
@testable import IchoiCore

final class ServerProfileTests: XCTestCase {
    func testHTTPSIsDefaultAndPrivateHTTPNeedsConsent() throws {
        XCTAssertNoThrow(try ServerURLPolicy.validate(url: URL(string: "https://music.example")!, privateHTTPConsent: false))
        XCTAssertThrowsError(try ServerURLPolicy.validate(url: URL(string: "http://music.example")!, privateHTTPConsent: true)) { XCTAssertEqual($0 as? ServerURLPolicyError, .publicHTTPNotAllowed) }
        XCTAssertThrowsError(try ServerURLPolicy.validate(url: URL(string: "http://192.168.1.12")!, privateHTTPConsent: false)) { XCTAssertEqual($0 as? ServerURLPolicyError, .privateHTTPConsentRequired) }
        XCTAssertNoThrow(try ServerURLPolicy.validate(url: URL(string: "http://[::1]")!, privateHTTPConsent: true))
        XCTAssertThrowsError(try ServerURLPolicy.validate(url: URL(string: "https://music.example/path")!, privateHTTPConsent: false))
        XCTAssertThrowsError(try ServerURLPolicy.validate(url: URL(string: "https://user@music.example")!, privateHTTPConsent: false))
    }

    func testProfileSeparation() throws {
        let defaults = UserDefaults(suiteName: "IchoiCoreTests")!
        defaults.removePersistentDomain(forName: "IchoiCoreTests")
        let store = ServerProfileStore(defaults: defaults)
        let first = try ServerProfile(name: "A", url: URL(string: "https://a.example")!)
        let second = try ServerProfile(name: "B", url: URL(string: "https://b.example")!)
        try store.save(first); try store.save(second)
        XCTAssertEqual(Set(store.profiles().map(\.id)), Set([first.id, second.id]))
        try store.remove(id: first.id)
        XCTAssertEqual(store.profiles().map(\.id), [second.id])
    }

    func testReportDetailsUseUnicodeScalars() throws {
        XCTAssertNoThrow(try ComplianceValidation.details(String(repeating: "🙂", count: 2_000)))
        XCTAssertThrowsError(try ComplianceValidation.details(String(repeating: "🙂", count: 2_001)))
    }

    func testDeletionRequiresExactHandle() throws {
        XCTAssertNoThrow(try ComplianceValidation.accountDeletionConfirmation("sam@example", handle: "sam@example"))
        XCTAssertThrowsError(try ComplianceValidation.accountDeletionConfirmation("Sam@example", handle: "sam@example"))
    }
}
