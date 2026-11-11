import XCTest
@testable import IchoiCore

final class MediaByteRangeTests: XCTestCase {
    func testRequestsStayBoundedForLargeBooksAndNearIntegerLimit() {
        XCTAssertEqual(MediaByteRange.chunkEnd(start: 0, exclusiveEnd: 10_000_000_000), 524_287)
        XCTAssertEqual(MediaByteRange.chunkEnd(start: 100, exclusiveEnd: 105), 104)
        XCTAssertEqual(MediaByteRange.chunkEnd(start: Int64.max - 10, exclusiveEnd: Int64.max), Int64.max - 1)
    }
    func testValidPartialAndFinalResponses() {
        XCTAssertEqual(MediaByteRange.parse("bytes 1024-2047/9000000000", requestedStart: 1024, requestedEnd: 2047)?.total, 9_000_000_000)
        XCTAssertEqual(MediaByteRange.parse("bytes 1024-1499/1500", requestedStart: 1024, requestedEnd: 2047)?.end, 1499)
    }
    func testRejectsIgnoredRangesTruncatedHeadersAndUnexpectedOffsets() {
        for value in [nil, "bytes */1200", "bytes 0-2047/9000", "bytes 1024-999/9000", "bytes 1024-9999/9000", "bytes 1024-9999999/10000000"] {
            XCTAssertNil(MediaByteRange.parse(value, requestedStart: 1024, requestedEnd: 2047))
        }
    }
}
