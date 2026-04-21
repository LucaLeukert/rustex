import Combine
import Foundation
import RustexGenerated

enum ExampleError: Error, CustomStringConvertible {
  case missingDeploymentUrl
  case missingArgument(String)
  case unknownCommand(String)
  case subscriptionFailed(String)

  var description: String {
    switch self {
    case .missingDeploymentUrl:
      return "missing Convex deployment URL; pass --deployment-url or set CONVEX_URL"
    case .missingArgument(let name):
      return "missing required argument \(name)"
    case .unknownCommand(let command):
      return "unknown command \(command)"
    case .subscriptionFailed(let message):
      return "subscription failed: \(message)"
    }
  }
}

struct Options {
  var deploymentUrl: String?
  var command: Command
}

enum Command {
  case add(author: String, body: String)
  case list
  case watch(updates: Int)
}

@main
struct RustexSwiftExample {
  static func main() async {
    do {
      loadExampleEnv()
      let options = try parseOptions(Array(CommandLine.arguments.dropFirst()))
      let deploymentUrl = options.deploymentUrl ?? ProcessInfo.processInfo.environment["CONVEX_URL"]
      guard let deploymentUrl else {
        throw ExampleError.missingDeploymentUrl
      }

      switch options.command {
      case .add(let author, let body):
        try await addMessage(deploymentUrl: deploymentUrl, author: author, body: body)
      case .list:
        try await listMessages(deploymentUrl: deploymentUrl)
      case .watch(let updates):
        try watchMessages(deploymentUrl: deploymentUrl, updates: updates)
      }
    } catch {
      fputs("\(error)\n", stderr)
      Foundation.exit(1)
    }
  }
}

func addMessage(deploymentUrl: String, author: String, body: String) async throws {
  let client = RustexClient(deploymentUrl: deploymentUrl)
  let id = try await client.mutation(API.Messages.add(author: author, body: body))
  print("inserted message id: \(id.rawValue)")
}

func listMessages(deploymentUrl: String) async throws {
  let client = RustexClient(deploymentUrl: deploymentUrl)
  let messages = try await client.query(API.Messages.collect())
  printMessages("messages", messages)
}

func watchMessages(deploymentUrl: String, updates: Int) throws {
  let client = RustexClient(deploymentUrl: deploymentUrl)
  let finished = DispatchSemaphore(value: 0)
  var received = 0
  var subscription: AnyCancellable?

  subscription = client.subscribe(API.Messages.collect()).sink(
    receiveCompletion: { completion in
      if case .failure(let error) = completion {
        fputs("\(ExampleError.subscriptionFailed(String(describing: error)))\n", stderr)
      }
      finished.signal()
    },
    receiveValue: { messages in
      received += 1
      print("update #\(received)")
      printMessages("messages", messages)
      if received >= updates {
        subscription?.cancel()
        finished.signal()
      }
    }
  )

  finished.wait()
}

func printMessages(_ label: String, _ messages: [CollectResponseItem]) {
  print("\(label): \(messages.count)")
  for message in messages {
    print("- \(message.id.rawValue) [\(message.creationTime)] \(message.author): \(message.body)")
  }
}

func parseOptions(_ args: [String]) throws -> Options {
  var deploymentUrl: String?
  var rest: [String] = []
  var index = 0

  while index < args.count {
    let arg = args[index]
    if arg == "--deployment-url" {
      index += 1
      guard index < args.count else {
        throw ExampleError.missingArgument("--deployment-url")
      }
      deploymentUrl = args[index]
    } else {
      rest.append(arg)
    }
    index += 1
  }

  guard let command = rest.first else {
    return Options(deploymentUrl: deploymentUrl, command: .list)
  }

  switch command {
  case "add":
    let author = try value(after: "--author", in: rest)
    let body = try value(after: "--body", in: rest)
    return Options(deploymentUrl: deploymentUrl, command: .add(author: author, body: body))
  case "list":
    return Options(deploymentUrl: deploymentUrl, command: .list)
  case "watch":
    let updates = Int((try? value(after: "--updates", in: rest)) ?? "3") ?? 3
    return Options(deploymentUrl: deploymentUrl, command: .watch(updates: updates))
  default:
    throw ExampleError.unknownCommand(command)
  }
}

func value(after flag: String, in args: [String]) throws -> String {
  guard let index = args.firstIndex(of: flag), args.indices.contains(index + 1) else {
    throw ExampleError.missingArgument(flag)
  }
  return args[index + 1]
}

func loadExampleEnv() {
  let manifest = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
  let envURL = manifest.deletingLastPathComponent().appendingPathComponent(".env.local")
  guard let contents = try? String(contentsOf: envURL) else {
    return
  }

  for line in contents.split(whereSeparator: \.isNewline) {
    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty || trimmed.hasPrefix("#") {
      continue
    }
    let parts = trimmed.split(separator: "=", maxSplits: 1).map(String.init)
    if parts.count == 2 {
      setenv(parts[0], parts[1], 0)
    }
  }
}
