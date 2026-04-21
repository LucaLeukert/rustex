use rustex_project::SwiftTargetConfig;

pub fn runtime_files(config: &SwiftTargetConfig) -> Vec<(&'static str, String)> {
    vec![
        ("RustexSpecs.swift", specs_swift()),
        ("RustexErrors.swift", errors_swift()),
        ("RustexEncoding.swift", encoding_swift()),
        ("RustexSupportTypes.swift", support_types_swift()),
        ("RustexClient.swift", client_swift(config)),
        ("RustexLogging.swift", logging_swift()),
    ]
}

fn specs_swift() -> String {
    r#"import ConvexMobile
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
"#
    .into()
}

fn errors_swift() -> String {
    r#"import ConvexMobile
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
"#
    .into()
}

fn encoding_swift() -> String {
    r#"import ConvexMobile
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
"#
    .into()
}

fn support_types_swift() -> String {
    r#"import ConvexMobile
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
"#
    .into()
}

fn client_swift(config: &SwiftTargetConfig) -> String {
    format!(
        r#"import Combine
import ConvexMobile
import Foundation

public final class {client} {{
  public let raw: ConvexClient

  public init(deploymentUrl: String) {{
    self.raw = ConvexClient(deploymentUrl: deploymentUrl)
  }}

  public init(_ raw: ConvexClient) {{
    self.raw = raw
  }}

  public func query<Q: RustexQuerySpec>(
    _ query: Q.Type,
    args: Q.Args
  ) async throws -> Q.Output {{
    try await withCheckedThrowingContinuation {{ continuation in
      var didResume = false
      var cancellable: AnyCancellable?
      cancellable = subscribe(query, args: args).first().sink(
        receiveCompletion: {{ completion in
          guard !didResume else {{ return }}
          didResume = true
          switch completion {{
          case .finished:
            continuation.resume(throwing: RustexRuntimeError.emptySubscription)
          case .failure(let error):
            continuation.resume(throwing: error)
          }}
          cancellable?.cancel()
        }},
        receiveValue: {{ value in
          guard !didResume else {{ return }}
          didResume = true
          continuation.resume(returning: value)
          cancellable?.cancel()
        }}
      )
    }}
  }}

  public func query<Q: RustexQuerySpec>(_ query: Q.Type) async throws -> Q.Output
  where Q.Args == RustexNoArgs {{
    try await self.query(query, args: RustexNoArgs())
  }}

  public func query<Q: RustexQuerySpec>(_ call: RustexQueryCall<Q>) async throws -> Q.Output {{
    try await self.query(Q.self, args: call.args)
  }}

  public func subscribe<Q: RustexQuerySpec>(
    _ query: Q.Type,
    args: Q.Args
  ) -> AnyPublisher<Q.Output, RustexRuntimeError> {{
    do {{
      return raw.subscribe(to: Q.path, with: try args.convexArgs(), yielding: Q.Output.self)
        .mapError {{ RustexRuntimeError.convex($0) }}
        .eraseToAnyPublisher()
    }} catch {{
      return Fail(error: RustexRuntimeError.encodingFailed(String(describing: error)))
        .eraseToAnyPublisher()
    }}
  }}

  public func subscribe<Q: RustexQuerySpec>(_ query: Q.Type) -> AnyPublisher<Q.Output, RustexRuntimeError>
  where Q.Args == RustexNoArgs {{
    subscribe(query, args: RustexNoArgs())
  }}

  public func subscribe<Q: RustexQuerySpec>(_ call: RustexQueryCall<Q>) -> AnyPublisher<Q.Output, RustexRuntimeError> {{
    subscribe(Q.self, args: call.args)
  }}

  public func mutation<M: RustexMutationSpec>(
    _ mutation: M.Type,
    args: M.Args
  ) async throws -> M.Output {{
    let encodedArgs: [String: ConvexEncodable?]
    do {{
      encodedArgs = try args.convexArgs()
    }} catch {{
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }}
    do {{
      return try await raw.mutation(M.path, with: encodedArgs)
    }} catch let error as ClientError {{
      throw RustexRuntimeError.convex(error)
    }} catch {{
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }}
  }}

  public func mutation<M: RustexMutationSpec>(_ mutation: M.Type) async throws -> M.Output
  where M.Args == RustexNoArgs {{
    try await self.mutation(mutation, args: RustexNoArgs())
  }}

  public func mutation<M: RustexMutationSpec>(_ call: RustexMutationCall<M>) async throws -> M.Output {{
    try await self.mutation(M.self, args: call.args)
  }}

  public func mutation<M: RustexMutationSpec>(
    _ mutation: M.Type,
    args: M.Args
  ) async throws
  where M.Output == RustexVoid {{
    let encodedArgs: [String: ConvexEncodable?]
    do {{
      encodedArgs = try args.convexArgs()
    }} catch {{
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }}
    do {{
      try await raw.mutation(M.path, with: encodedArgs)
    }} catch let error as ClientError {{
      throw RustexRuntimeError.convex(error)
    }} catch {{
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }}
  }}

  public func mutation<M: RustexMutationSpec>(_ call: RustexMutationCall<M>) async throws
  where M.Output == RustexVoid {{
    try await self.mutation(M.self, args: call.args)
  }}

  public func action<A: RustexActionSpec>(
    _ action: A.Type,
    args: A.Args
  ) async throws -> A.Output {{
    let encodedArgs: [String: ConvexEncodable?]
    do {{
      encodedArgs = try args.convexArgs()
    }} catch {{
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }}
    do {{
      return try await raw.action(A.path, with: encodedArgs)
    }} catch let error as ClientError {{
      throw RustexRuntimeError.convex(error)
    }} catch {{
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }}
  }}

  public func action<A: RustexActionSpec>(_ action: A.Type) async throws -> A.Output
  where A.Args == RustexNoArgs {{
    try await self.action(action, args: RustexNoArgs())
  }}

  public func action<A: RustexActionSpec>(_ call: RustexActionCall<A>) async throws -> A.Output {{
    try await self.action(A.self, args: call.args)
  }}

  public func action<A: RustexActionSpec>(
    _ action: A.Type,
    args: A.Args
  ) async throws
  where A.Output == RustexVoid {{
    let encodedArgs: [String: ConvexEncodable?]
    do {{
      encodedArgs = try args.convexArgs()
    }} catch {{
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }}
    do {{
      try await raw.action(A.path, with: encodedArgs)
    }} catch let error as ClientError {{
      throw RustexRuntimeError.convex(error)
    }} catch {{
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }}
  }}

  public func action<A: RustexActionSpec>(_ call: RustexActionCall<A>) async throws
  where A.Output == RustexVoid {{
    try await self.action(A.self, args: call.args)
  }}

  public func watchWebSocketState() -> AnyPublisher<WebSocketState, Never> {{
    raw.watchWebSocketState()
  }}
}}
"#,
        client = config.client_facade_name
    )
}

fn logging_swift() -> String {
    r#"import ConvexMobile
import Foundation

public func initRustexLogging() {
  initConvexLogging()
}
"#
    .into()
}
