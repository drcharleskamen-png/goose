import Foundation

public actor ACPClient {
    public let notifications: AsyncStream<ACPNotification>
    public let agentRequests: AsyncStream<ACPAgentRequest>

    private let transport: any ACPTransport
    private let autoCancelPermissionRequests: Bool
    private let notificationContinuation: AsyncStream<ACPNotification>.Continuation
    private let agentRequestContinuation: AsyncStream<ACPAgentRequest>.Continuation
    private var nextID = 1
    private var receiveTask: Task<Void, Never>?
    private var pending: [String: CheckedContinuation<JSONValue, Error>] = [:]

    public init(
        transport: any ACPTransport,
        autoCancelPermissionRequests: Bool = true
    ) {
        let notificationStream = AsyncStream<ACPNotification>.makeStream()
        self.notifications = notificationStream.stream
        self.notificationContinuation = notificationStream.continuation

        let agentRequestStream = AsyncStream<ACPAgentRequest>.makeStream()
        self.agentRequests = agentRequestStream.stream
        self.agentRequestContinuation = agentRequestStream.continuation

        self.transport = transport
        self.autoCancelPermissionRequests = autoCancelPermissionRequests
    }

    deinit {
        receiveTask?.cancel()
        notificationContinuation.finish()
        agentRequestContinuation.finish()
    }

    public func start() {
        guard receiveTask == nil else {
            return
        }
        receiveTask = Task { [weak self] in
            await self?.receiveLoop()
        }
    }

    public func close() async {
        receiveTask?.cancel()
        receiveTask = nil
        await transport.close()
        failAllPending(with: ACPClientError.closed)
        notificationContinuation.finish()
        agentRequestContinuation.finish()
    }

    @discardableResult
    public func initialize(
        clientName: String = "goose-mobile-client",
        clientVersion: String = "0.1.0"
    ) async throws -> InitializeResponse {
        start()
        let params: JSONValue = .object([
            "protocolVersion": .number(1),
            "clientCapabilities": .object([
                "_meta": .object([
                    "goose": .object([
                        "customNotifications": .bool(true),
                        "recipeParameterRequests": .bool(false),
                    ]),
                ]),
            ]),
            "clientInfo": .object([
                "name": .string(clientName),
                "version": .string(clientVersion),
            ]),
        ])
        return try await request(method: ACPMethod.initialize, params: params)
            .decode(InitializeResponse.self)
    }

    public func listSessions(
        cursor: String? = nil,
        query: String? = nil,
        types: [String] = ["user", "scheduled"]
    ) async throws -> ListSessionsResponse {
        var meta: [String: JSONValue] = [
            "types": .array(types.map(JSONValue.string)),
            "goose.includeLastMessageSnippet": .bool(true),
        ]
        if let query, !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            meta["query"] = .string(query)
        }

        var params: [String: JSONValue] = ["_meta": .object(meta)]
        if let cursor {
            params["cursor"] = .string(cursor)
        }

        return try await request(method: ACPMethod.listSessions, params: .object(params))
            .decode(ListSessionsResponse.self)
    }

    public func sessionInfo(sessionID: String) async throws -> SessionInfo {
        let result = try await request(
            method: ACPMethod.gooseSessionInfo,
            params: .object(["sessionId": .string(sessionID)])
        )
        return try result["session"]?.decode(SessionInfo.self)
            ?? { throw ACPClientError.protocolViolation("Missing session in Goose session info response") }()
    }

    @discardableResult
    public func loadSession(_ session: SessionInfo) async throws -> LoadSessionResponse {
        try await loadSession(sessionID: session.sessionID, cwd: session.cwd)
    }

    @discardableResult
    public func loadSession(sessionID: String, cwd: String) async throws -> LoadSessionResponse {
        let params: JSONValue = .object([
            "sessionId": .string(sessionID),
            "cwd": .string(cwd),
            "mcpServers": .array([]),
        ])
        return try await request(method: ACPMethod.loadSession, params: params)
            .decode(LoadSessionResponse.self)
    }

    @discardableResult
    public func prompt(sessionID: String, text: String) async throws -> PromptResponse {
        let params: JSONValue = .object([
            "sessionId": .string(sessionID),
            "prompt": .array([
                .object([
                    "type": .string("text"),
                    "text": .string(text),
                ]),
            ]),
        ])
        return try await request(method: ACPMethod.prompt, params: params)
            .decode(PromptResponse.self)
    }

    public func cancel(sessionID: String) async throws {
        try await notify(
            method: ACPMethod.cancel,
            params: .object(["sessionId": .string(sessionID)])
        )
    }

    @discardableResult
    public func request(method: String, params: JSONValue? = nil) async throws -> JSONValue {
        start()
        let id = JSONRPCID.number(nextID)
        nextID += 1
        let request = JSONRPCRequest(id: id, method: method, params: params)
        let message = try JSONValue.encoded(request)
        let key = id.correlationKey
        let transport = self.transport

        return try await withCheckedThrowingContinuation { continuation in
            pending[key] = continuation
            Task {
                do {
                    try await transport.send(message)
                } catch {
                    self.resolvePending(key: key, result: .failure(error))
                }
            }
        }
    }

    public func notify(method: String, params: JSONValue? = nil) async throws {
        let request = JSONRPCRequest(method: method, params: params)
        try await transport.send(JSONValue.encoded(request))
    }

    public func respond(to request: ACPAgentRequest, result: JSONValue) async throws {
        try await sendResponse(id: request.id, result: result)
    }

    public func reject(to request: ACPAgentRequest, code: Int, message: String) async throws {
        try await sendError(id: request.id, code: code, message: message)
    }

    private func receiveLoop() async {
        do {
            while !Task.isCancelled {
                guard let value = try await transport.receive() else {
                    break
                }
                try await handleIncoming(value)
            }
        } catch {
            failAllPending(with: error)
        }
    }

    private func handleIncoming(_ value: JSONValue) async throws {
        switch try JSONRPCIncomingMessage.decode(value) {
        case let .response(response):
            let key = response.id.correlationKey
            if let error = response.error {
                resolvePending(key: key, result: .failure(ACPClientError.remote(error)))
            } else {
                resolvePending(key: key, result: .success(response.result ?? .object([:])))
            }

        case let .notification(notification):
            notificationContinuation.yield(
                ACPNotification(method: notification.method, params: notification.params)
            )

        case let .request(request):
            guard let id = request.id else {
                throw ACPClientError.protocolViolation("Inbound request is missing id")
            }
            let agentRequest = ACPAgentRequest(id: id, method: request.method, params: request.params)
            agentRequestContinuation.yield(agentRequest)
            try await handleDefaultAgentRequest(agentRequest)
        }
    }

    private func handleDefaultAgentRequest(_ request: ACPAgentRequest) async throws {
        if autoCancelPermissionRequests && request.method == ACPMethod.requestPermission {
            try await sendResponse(
                id: request.id,
                result: .object([
                    "outcome": .object(["outcome": .string("cancelled")]),
                ])
            )
            return
        }

        if autoCancelPermissionRequests {
            try await sendError(
                id: request.id,
                code: -32601,
                message: "Unsupported mobile client method: \(request.method)"
            )
        }
    }

    private func sendResponse(id: JSONRPCID, result: JSONValue) async throws {
        let response = JSONRPCResponse(id: id, result: result)
        try await transport.send(JSONValue.encoded(response))
    }

    private func sendError(id: JSONRPCID, code: Int, message: String) async throws {
        let response = JSONRPCResponse(
            id: id,
            error: JSONRPCError(code: code, message: message)
        )
        try await transport.send(JSONValue.encoded(response))
    }

    private func resolvePending(key: String, result: Result<JSONValue, Error>) {
        guard let continuation = pending.removeValue(forKey: key) else {
            return
        }
        continuation.resume(with: result)
    }

    private func failAllPending(with error: Error) {
        let continuations = pending.values
        pending.removeAll()
        for continuation in continuations {
            continuation.resume(throwing: error)
        }
    }
}
