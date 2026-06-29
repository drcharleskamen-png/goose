import Foundation
import GooseMobileClient

@main
enum GooseMobileDemo {
    static func main() async {
        do {
            try await run()
        } catch {
            fputs("error: \(error)\n", stderr)
            Foundation.exit(1)
        }
    }

    private static func run() async throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        if arguments.contains("--help") || arguments.contains("-h") {
            printUsage()
            return
        }

        let urlString = value(after: "--url", in: arguments)
            ?? ProcessInfo.processInfo.environment["GOOSE_ACP_WS_URL"]
            ?? "ws://127.0.0.1:3284/acp"
        let token = value(after: "--token", in: arguments)
            ?? ProcessInfo.processInfo.environment["GOOSE_SERVER__SECRET_KEY"]

        guard let url = URL(string: urlString) else {
            throw DemoError.invalidURL(urlString)
        }

        let command = positionalArguments(from: arguments).first ?? "list"
        let transport = WebSocketACPTransport(url: url, token: token)
        let client = ACPClient(transport: transport)
        defer {
            Task { await client.close() }
        }

        let initializeResponse = try await client.initialize()
        let agentName = initializeResponse.agentInfo?.name ?? "unknown-agent"
        print("connected: \(agentName) protocol=\(initializeResponse.protocolVersion)")

        switch command {
        case "list":
            let response = try await client.listSessions()
            if response.sessions.isEmpty {
                print("no sessions")
            } else {
                for session in response.sessions {
                    let title = session.title ?? "(untitled)"
                    print("\(session.sessionID)\t\(title)\t\(session.cwd)")
                }
            }

        case "prompt":
            let positional = positionalArguments(from: arguments)
            guard positional.count >= 3 else {
                throw DemoError.invalidArguments("prompt requires <session-id> and text")
            }
            let sessionID = positional[1]
            let text = positional.dropFirst(2).joined(separator: " ")
            let session = try await client.sessionInfo(sessionID: sessionID)
            try await client.loadSession(session)
            let response = try await client.prompt(sessionID: sessionID, text: text)
            print("stopReason: \(response.stopReason)")

        default:
            throw DemoError.invalidArguments("unknown command: \(command)")
        }
    }

    private static func value(after flag: String, in arguments: [String]) -> String? {
        guard let index = arguments.firstIndex(of: flag) else {
            return nil
        }
        let nextIndex = arguments.index(after: index)
        guard arguments.indices.contains(nextIndex) else {
            return nil
        }
        return arguments[nextIndex]
    }

    private static func positionalArguments(from arguments: [String]) -> [String] {
        var result: [String] = []
        var skipNext = false
        for argument in arguments {
            if skipNext {
                skipNext = false
                continue
            }
            if argument == "--url" || argument == "--token" {
                skipNext = true
                continue
            }
            if argument.hasPrefix("-") {
                continue
            }
            result.append(argument)
        }
        return result
    }

    private static func printUsage() {
        print("""
        usage:
          goose-mobile-demo [--url ws://127.0.0.1:3284/acp] [--token secret] list
          goose-mobile-demo [--url ws://127.0.0.1:3284/acp] [--token secret] prompt <session-id> <text>
        """)
    }
}

enum DemoError: Error, CustomStringConvertible {
    case invalidURL(String)
    case invalidArguments(String)

    var description: String {
        switch self {
        case let .invalidURL(value):
            return "invalid URL: \(value)"
        case let .invalidArguments(message):
            return message
        }
    }
}
