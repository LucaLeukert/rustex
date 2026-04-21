import ConvexMobile
import Foundation
@_exported import RustexRuntime

public struct AddArgs: Codable, Equatable, RustexConvexArgs {
  public let author: String
  public let body: String

  enum CodingKeys: String, CodingKey {
    case author
    case body
  }

  public init(author: String, body: String) {
    self.author = author
    self.body = body
  }

  public func convexArgs() throws -> [String: ConvexEncodable?] {
    [
      "author": try author.rustexConvexValue(),
      "body": try body.rustexConvexValue(),
    ]
  }
}

extension AddArgs: ConvexEncodable {}
extension AddArgs: RustexConvexValueConvertible {
  public func rustexConvexValue() throws -> ConvexEncodable? { self }
}

public struct CollectResponseItem: Decodable, Equatable {
  public let id: MessagesId
  @ConvexFloat
  public var creationTime: Double
  public let author: String
  public let body: String

  enum CodingKeys: String, CodingKey {
    case id = "_id"
    case creationTime = "_creationTime"
    case author
    case body
  }

  public init(id: MessagesId, creationTime: Double, author: String, body: String) {
    self.id = id
    self.creationTime = creationTime
    self.author = author
    self.body = body
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.id = try container.decode(MessagesId.self, forKey: .id)
    self.creationTime = try container.decode(ConvexFloat<Double>.self, forKey: .creationTime).wrappedValue
    self.author = try container.decode(String.self, forKey: .author)
    self.body = try container.decode(String.self, forKey: .body)
  }
}

public enum API {
  public enum Messages {
    public enum Add: RustexMutationSpec {
      public typealias Args = AddArgs
      public typealias Output = MessagesId
      public static let path = "messages:add"
    }

    public static func add(author: String, body: String) -> RustexMutationCall<Add> {
      RustexMutationCall(args: AddArgs(author: author, body: body))
    }

    public enum Collect: RustexQuerySpec {
      public typealias Args = RustexNoArgs
      public typealias Output = [CollectResponseItem]
      public static let path = "messages:collect"
    }

    public static func collect() -> RustexQueryCall<Collect> {
      RustexQueryCall(args: RustexNoArgs())
    }

  }

}
