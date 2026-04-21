import Combine
import ConvexMobile
import Foundation

public final class RustexClient {
  public let raw: ConvexClient

  public init(deploymentUrl: String) {
    self.raw = ConvexClient(deploymentUrl: deploymentUrl)
  }

  public init(_ raw: ConvexClient) {
    self.raw = raw
  }

  public func query<Q: RustexQuerySpec>(
    _ query: Q.Type,
    args: Q.Args
  ) async throws -> Q.Output {
    try await withCheckedThrowingContinuation { continuation in
      var didResume = false
      var cancellable: AnyCancellable?
      cancellable = subscribe(query, args: args).first().sink(
        receiveCompletion: { completion in
          guard !didResume else { return }
          didResume = true
          switch completion {
          case .finished:
            continuation.resume(throwing: RustexRuntimeError.emptySubscription)
          case .failure(let error):
            continuation.resume(throwing: error)
          }
          cancellable?.cancel()
        },
        receiveValue: { value in
          guard !didResume else { return }
          didResume = true
          continuation.resume(returning: value)
          cancellable?.cancel()
        }
      )
    }
  }

  public func query<Q: RustexQuerySpec>(_ query: Q.Type) async throws -> Q.Output
  where Q.Args == RustexNoArgs {
    try await self.query(query, args: RustexNoArgs())
  }

  public func query<Q: RustexQuerySpec>(_ call: RustexQueryCall<Q>) async throws -> Q.Output {
    try await self.query(Q.self, args: call.args)
  }

  public func subscribe<Q: RustexQuerySpec>(
    _ query: Q.Type,
    args: Q.Args
  ) -> AnyPublisher<Q.Output, RustexRuntimeError> {
    do {
      return raw.subscribe(to: Q.path, with: try args.convexArgs(), yielding: Q.Output.self)
        .mapError { RustexRuntimeError.convex($0) }
        .eraseToAnyPublisher()
    } catch {
      return Fail(error: RustexRuntimeError.encodingFailed(String(describing: error)))
        .eraseToAnyPublisher()
    }
  }

  public func subscribe<Q: RustexQuerySpec>(_ query: Q.Type) -> AnyPublisher<Q.Output, RustexRuntimeError>
  where Q.Args == RustexNoArgs {
    subscribe(query, args: RustexNoArgs())
  }

  public func subscribe<Q: RustexQuerySpec>(_ call: RustexQueryCall<Q>) -> AnyPublisher<Q.Output, RustexRuntimeError> {
    subscribe(Q.self, args: call.args)
  }

  public func mutation<M: RustexMutationSpec>(
    _ mutation: M.Type,
    args: M.Args
  ) async throws -> M.Output {
    let encodedArgs: [String: ConvexEncodable?]
    do {
      encodedArgs = try args.convexArgs()
    } catch {
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }
    do {
      return try await raw.mutation(M.path, with: encodedArgs)
    } catch let error as ClientError {
      throw RustexRuntimeError.convex(error)
    } catch {
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }
  }

  public func mutation<M: RustexMutationSpec>(_ mutation: M.Type) async throws -> M.Output
  where M.Args == RustexNoArgs {
    try await self.mutation(mutation, args: RustexNoArgs())
  }

  public func mutation<M: RustexMutationSpec>(_ call: RustexMutationCall<M>) async throws -> M.Output {
    try await self.mutation(M.self, args: call.args)
  }

  public func mutation<M: RustexMutationSpec>(
    _ mutation: M.Type,
    args: M.Args
  ) async throws
  where M.Output == RustexVoid {
    let encodedArgs: [String: ConvexEncodable?]
    do {
      encodedArgs = try args.convexArgs()
    } catch {
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }
    do {
      try await raw.mutation(M.path, with: encodedArgs)
    } catch let error as ClientError {
      throw RustexRuntimeError.convex(error)
    } catch {
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }
  }

  public func mutation<M: RustexMutationSpec>(_ call: RustexMutationCall<M>) async throws
  where M.Output == RustexVoid {
    try await self.mutation(M.self, args: call.args)
  }

  public func action<A: RustexActionSpec>(
    _ action: A.Type,
    args: A.Args
  ) async throws -> A.Output {
    let encodedArgs: [String: ConvexEncodable?]
    do {
      encodedArgs = try args.convexArgs()
    } catch {
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }
    do {
      return try await raw.action(A.path, with: encodedArgs)
    } catch let error as ClientError {
      throw RustexRuntimeError.convex(error)
    } catch {
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }
  }

  public func action<A: RustexActionSpec>(_ action: A.Type) async throws -> A.Output
  where A.Args == RustexNoArgs {
    try await self.action(action, args: RustexNoArgs())
  }

  public func action<A: RustexActionSpec>(_ call: RustexActionCall<A>) async throws -> A.Output {
    try await self.action(A.self, args: call.args)
  }

  public func action<A: RustexActionSpec>(
    _ action: A.Type,
    args: A.Args
  ) async throws
  where A.Output == RustexVoid {
    let encodedArgs: [String: ConvexEncodable?]
    do {
      encodedArgs = try args.convexArgs()
    } catch {
      throw RustexRuntimeError.encodingFailed(String(describing: error))
    }
    do {
      try await raw.action(A.path, with: encodedArgs)
    } catch let error as ClientError {
      throw RustexRuntimeError.convex(error)
    } catch {
      throw RustexRuntimeError.decodingFailed(String(describing: error))
    }
  }

  public func action<A: RustexActionSpec>(_ call: RustexActionCall<A>) async throws
  where A.Output == RustexVoid {
    try await self.action(A.self, args: call.args)
  }

  public func watchWebSocketState() -> AnyPublisher<WebSocketState, Never> {
    raw.watchWebSocketState()
  }
}
