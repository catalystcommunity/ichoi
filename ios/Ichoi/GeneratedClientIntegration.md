# Generated Swift client integration

The Xcode project references these generated files directly:

- `server/generated/swift-client/Client.swift`
- `server/generated/swift-client/ClientAsync.swift`
- `server/generated/swift-client/Codec.swift`
- `server/generated/swift-client/Types.swift`

Run `./tools.sh gen` from the repository root after a CSIL schema change. Do not copy or
edit these files in `ios/`. The project build uses the generated `AsyncCsilTransport`
protocol from `ClientAsync.swift` and handwritten `CSILHTTPTransport` supplies network I/O.

csilgen 0.2.7 writes some optional enum defaults as string literals. The handwritten
`GeneratedClientCompatibility.swift` file gives the affected enums the required Swift
literal conformance. Remove this compatibility file when the generator emits enum cases.
