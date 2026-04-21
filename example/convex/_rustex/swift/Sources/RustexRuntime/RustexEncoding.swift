import ConvexMobile
import Foundation

public protocol RustexConvexValueConvertible {
  func rustexConvexValue() throws -> ConvexEncodable?
}

extension String: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Bool: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Int: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Int32: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Int64: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Float: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Double: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

extension Optional: RustexConvexValueConvertible where Wrapped: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? {
    switch self {
    case .some(let value):
      return try value.rustexConvexValue()
    case .none:
      return nil
    }
  }
}

extension Array: RustexConvexValueConvertible where Element: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? {
    try self.map { try $0.rustexConvexValue() }
  }
}

extension Dictionary: RustexConvexValueConvertible where Key == String, Value: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? {
    var encoded: [String: ConvexEncodable?] = [:]
    for (key, value) in self {
      encoded[key] = try value.rustexConvexValue()
    }
    return encoded
  }
}
