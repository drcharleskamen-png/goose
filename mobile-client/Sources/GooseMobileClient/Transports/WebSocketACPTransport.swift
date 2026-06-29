import Foundation

public final class WebSocketACPTransport: ACPTransport, Sendable {
    private let state: WebSocketACPTransportState

    public init(url: URL, token: String? = nil, session: URLSession = .shared) {
        self.state = WebSocketACPTransportState(
            url: Self.urlWithToken(url, token: token),
            session: session
        )
    }

    public func send(_ message: JSONValue) async throws {
        let data = try JSONEncoder().encode(message)
        let text = String(decoding: data, as: UTF8.self)
        try await state.send(text)
    }

    public func receive() async throws -> JSONValue? {
        guard let data = try await state.receiveData() else {
            return nil
        }
        return try JSONDecoder().decode(JSONValue.self, from: data)
    }

    public func close() async {
        await state.close()
    }

    private static func urlWithToken(_ url: URL, token: String?) -> URL {
        guard let token, !token.isEmpty else {
            return url
        }

        guard var components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return url
        }

        var queryItems = components.queryItems ?? []
        if !queryItems.contains(where: { $0.name == "token" }) {
            queryItems.append(URLQueryItem(name: "token", value: token))
        }
        components.queryItems = queryItems
        return components.url ?? url
    }
}

private actor WebSocketACPTransportState {
    private let url: URL
    private let session: URLSession
    private var task: URLSessionWebSocketTask?

    init(url: URL, session: URLSession) {
        self.url = url
        self.session = session
    }

    func send(_ text: String) async throws {
        try await webSocketTask().send(.string(text))
    }

    func receiveData() async throws -> Data? {
        switch try await webSocketTask().receive() {
        case let .string(text):
            return Data(text.utf8)
        case let .data(data):
            return data
        @unknown default:
            return nil
        }
    }

    func close() {
        task?.cancel(with: .normalClosure, reason: nil)
        task = nil
    }

    private func webSocketTask() -> URLSessionWebSocketTask {
        if let task {
            return task
        }

        let task = session.webSocketTask(with: url)
        task.resume()
        self.task = task
        return task
    }
}
