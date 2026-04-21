import ConvexMobile
import Foundation

public protocol RustexFunctionSpec {
  associatedtype Args: RustexConvexArgs
  associatedtype Output: Decodable

  static var path: String { get }
}

public protocol RustexQuerySpec: RustexFunctionSpec {}
public protocol RustexMutationSpec: RustexFunctionSpec {}
public protocol RustexActionSpec: RustexFunctionSpec {}

public protocol RustexConvexArgs: Encodable {
  func convexArgs() throws -> [String: ConvexEncodable?]
}

public struct RustexNoArgs: Codable, Equatable, RustexConvexArgs {
  public init() {}

  public func convexArgs() throws -> [String: ConvexEncodable?] {
    [:]
  }
}

public struct RustexQueryCall<Q: RustexQuerySpec> {
  public let args: Q.Args

  public init(args: Q.Args) {
    self.args = args
  }
}

public struct RustexMutationCall<M: RustexMutationSpec> {
  public let args: M.Args

  public init(args: M.Args) {
    self.args = args
  }
}

public struct RustexActionCall<A: RustexActionSpec> {
  public let args: A.Args

  public init(args: A.Args) {
    self.args = args
  }
}
