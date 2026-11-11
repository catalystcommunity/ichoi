import XCTest
@testable import IchoiCore

final class AppMessagesTests: XCTestCase {
    @MainActor
    private final class Clock {
        var delays: [UInt64] = []
        var waits: [CheckedContinuation<Void, Error>] = []
        func sleep(_ duration: UInt64) async throws {
            try await withCheckedThrowingContinuation { continuation in
                delays.append(duration)
                waits.append(continuation)
            }
        }
        func finishNext() { waits.removeFirst().resume() }
    }

    @MainActor
    func testDefaultDurationAndAutomaticDismissal() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let clock = Clock()
        let messages = AppMessages(defaults: defaults, sleep: clock.sleep)
        messages.show("Playback failed")
        for _ in 0..<20 { await Task.yield() }
        XCTAssertEqual(clock.delays, [10_000_000_000])
        XCTAssertNotNil(messages.current)
        guard !clock.waits.isEmpty else { return }
        clock.finishNext()
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNil(messages.current)
    }

    @MainActor
    func testOldTimerCannotDismissReplacement() async {
        let clock = Clock()
        let messages = AppMessages(defaults: UserDefaults(suiteName: UUID().uuidString)!, sleep: clock.sleep)
        messages.show("Playback failed")
        let oldID = messages.current!.id
        for _ in 0..<20 { await Task.yield() }
        messages.show("Playback failed")
        for _ in 0..<20 { await Task.yield() }
        XCTAssertEqual(clock.waits.count, 2)
        guard clock.waits.count == 2 else { return }
        clock.finishNext()
        messages.dismiss(id: oldID)
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNotNil(messages.current)
        clock.finishNext()
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNil(messages.current)
    }

    @MainActor
    func testManualDismissalAndSavedDuration() async {
        let suite = UUID().uuidString
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let clock = Clock()
        let messages = AppMessages(defaults: defaults, sleep: clock.sleep)
        messages.durationSeconds = 30
        XCTAssertEqual(AppMessages(defaults: defaults).durationSeconds, 30)
        messages.show("Connection failed")
        for _ in 0..<20 { await Task.yield() }
        XCTAssertEqual(clock.delays, [30_000_000_000])
        messages.dismiss()
        XCTAssertNil(messages.current)
        guard !clock.waits.isEmpty else { return }
        clock.finishNext()
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNil(messages.current)
    }

    @MainActor
    func testBackgroundErrorWaitsUntilAppIsActive() async {
        let clock = Clock()
        let messages = AppMessages(defaults: UserDefaults(suiteName: UUID().uuidString)!, sleep: clock.sleep)
        messages.setActive(false)
        messages.show("Playback failed")
        for _ in 0..<20 { await Task.yield() }
        XCTAssertTrue(clock.waits.isEmpty)
        XCTAssertNotNil(messages.current)
        messages.setActive(true)
        for _ in 0..<20 { await Task.yield() }
        XCTAssertEqual(clock.delays, [10_000_000_000])
        guard !clock.waits.isEmpty else { return }
        clock.finishNext()
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNil(messages.current)
    }
}
