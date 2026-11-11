import Foundation
import Combine

@MainActor
public final class AppMessages: ObservableObject {
    public struct Message: Identifiable, Equatable {
        public let id = UUID()
        public let text: String
    }

    @Published public private(set) var current: Message?
    @Published public var durationSeconds: Int {
        didSet {
            let clamped = min(60, max(1, durationSeconds))
            if clamped != durationSeconds { durationSeconds = clamped }
            defaults.set(clamped, forKey: Self.durationKey)
        }
    }
    private static let durationKey = "ichoi.messages.durationSeconds"
    private let defaults: UserDefaults
    private let sleep: (UInt64) async throws -> Void
    private var expiry: Task<Void, Never>?
    private var active = true

    public init(defaults: UserDefaults = .standard,
                sleep: @escaping (UInt64) async throws -> Void = { try await Task.sleep(nanoseconds: $0) }) {
        self.defaults = defaults
        self.sleep = sleep
        durationSeconds = defaults.object(forKey: Self.durationKey) == nil ? 10 : min(60, max(1, defaults.integer(forKey: Self.durationKey)))
    }

    public func show(_ text: String) {
        expiry?.cancel()
        current = Message(text: text)
        scheduleExpiry()
    }

    public func dismiss(id: UUID? = nil) {
        if let id, current?.id != id { return }
        expiry?.cancel()
        expiry = nil
        current = nil
    }

    public func setActive(_ value: Bool) {
        guard active != value else { return }
        active = value
        expiry?.cancel()
        expiry = nil
        if active { scheduleExpiry() }
    }

    private func scheduleExpiry() {
        guard active, let message = current else { return }
        let delay = UInt64(durationSeconds) * 1_000_000_000
        let sleep = self.sleep
        expiry = Task { [weak self] in
            do { try await sleep(delay) } catch { return }
            guard !Task.isCancelled else { return }
            self?.dismiss(id: message.id)
        }
    }
}
