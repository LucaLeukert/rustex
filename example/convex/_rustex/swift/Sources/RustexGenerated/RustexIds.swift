import ConvexMobile
import Foundation
@_exported import RustexRuntime

public struct MessagesId: Codable, Hashable, ExpressibleByStringLiteral {
  public let rawValue: String

  public init(_ rawValue: String) {
    self.rawValue = rawValue
  }

  public init(stringLiteral value: String) {
    self.rawValue = value
  }

  public init(from decoder: Decoder) throws {
    self.rawValue = try decoder.singleValueContainer().decode(String.self)
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.singleValueContainer()
    try container.encode(rawValue)
  }
}

extension MessagesId: ConvexEncodable {}
extension MessagesId: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { rawValue }
}

