import ConvexMobile
import Foundation
@_exported import RustexRuntime

public struct MessagesDoc: Decodable, Equatable {
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

