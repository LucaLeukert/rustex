import ConvexMobile
import Foundation

public enum RustexRuntimeError: Error {
  case convex(ClientError)
  case invalidArgsShape
  case encodingFailed(String)
  case decodingFailed(String)
  case emptySubscription
}

extension RustexRuntimeError: CustomStringConvertible {
  public var description: String {
    switch self {
    case .convex(let error):
      return "Convex client error: \(error)"
    case .invalidArgsShape:
      return "Rustex arguments must encode to an object"
    case .encodingFailed(let message):
      return "Rustex argument encoding failed: \(message)"
    case .decodingFailed(let message):
      return "Rustex response decoding failed: \(message)"
    case .emptySubscription:
      return "Convex query subscription completed before yielding a value"
    }
  }
}
