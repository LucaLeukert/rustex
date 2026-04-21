import ConvexMobile
import Foundation

public struct RustexVoid: Decodable, Equatable {
  public init() {}

  public init(from decoder: Decoder) throws {
    if let container = try? decoder.singleValueContainer(), container.decodeNil() {
      self.init()
      return
    }
    self.init()
  }
}

public struct RustexNull: Codable, Equatable {
  public init() {}

  public init(from decoder: Decoder) throws {
    let container = try decoder.singleValueContainer()
    if !container.decodeNil() {
      throw DecodingError.typeMismatch(
        RustexNull.self,
        DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Expected null")
      )
    }
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.singleValueContainer()
    try container.encodeNil()
  }
}

extension RustexNull: ConvexEncodable {}
extension RustexNull: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { nil }
}

public enum AnyCodable: Codable, Equatable {
  case null
  case bool(Bool)
  case int(Int64)
  case double(Double)
  case string(String)
  case array([AnyCodable])
  case object([String: AnyCodable])

  public init(from decoder: Decoder) throws {
    let single = try decoder.singleValueContainer()
    if single.decodeNil() {
      self = .null
    } else if let value = try? single.decode(Bool.self) {
      self = .bool(value)
    } else if let value = try? single.decode(Int64.self) {
      self = .int(value)
    } else if let value = try? single.decode(Double.self) {
      self = .double(value)
    } else if let value = try? single.decode(String.self) {
      self = .string(value)
    } else if let value = try? single.decode([AnyCodable].self) {
      self = .array(value)
    } else {
      self = .object(try single.decode([String: AnyCodable].self))
    }
  }

  public func encode(to encoder: Encoder) throws {
    var single = encoder.singleValueContainer()
    switch self {
    case .null:
      try single.encodeNil()
    case .bool(let value):
      try single.encode(value)
    case .int(let value):
      try single.encode(value)
    case .double(let value):
      try single.encode(value)
    case .string(let value):
      try single.encode(value)
    case .array(let value):
      try single.encode(value)
    case .object(let value):
      try single.encode(value)
    }
  }
}

extension AnyCodable: ConvexEncodable {}
extension AnyCodable: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}
