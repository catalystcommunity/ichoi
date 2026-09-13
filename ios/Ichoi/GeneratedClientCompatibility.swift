// The generated Swift client writes enum defaults as string literals. These
// conformances let those generated initializers compile without changing the
// generated files. Remove this wrapper after csilgen emits enum case defaults.
extension Library: ExpressibleByStringLiteral {
    public init(stringLiteral value: String) {
        guard let result = Library(rawValue: value) else {
            preconditionFailure("Unknown generated Library default: \(value)")
        }
        self = result
    }
}

extension TranscodeCodec: ExpressibleByStringLiteral {
    public init(stringLiteral value: String) {
        guard let result = TranscodeCodec(rawValue: value) else {
            preconditionFailure("Unknown generated TranscodeCodec default: \(value)")
        }
        self = result
    }
}
