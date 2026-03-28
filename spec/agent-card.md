# Agent Card Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Agent Cards are JSON documents that describe an agent's capabilities, skills, and preferences. They enable **capability discovery before interaction** -- before sending a message to an agent, a client or another agent can fetch the Agent Card to learn what the agent can do, what content types it supports, whether it streams responses, what tools it exposes, and how it handles threading and memory.

Agent Cards serve two audiences. First, they provide structured metadata for programmatic consumption: other agents can read an Agent Card to decide whether a particular agent is the right one for a task. Second, they provide human-readable information for client UIs: a messaging client can display an agent's name, description, and capabilities in a profile view.

AINM Agent Cards are a **superset of A2A Agent Cards** (per D-09). The top-level fields follow the A2A Agent Card schema exactly, ensuring that A2A-compatible clients can read AINM Agent Cards without modification. All AINM-specific extensions live under a namespaced `ainm` top-level object, which A2A validators skip as an additional property (see Pitfall 5 in Phase 2 research). This means an AINM Agent Card is simultaneously a valid A2A Agent Card and a richer AINM capability manifest.

Human participants (type: `"human"` in the identity record) do not have Agent Cards. The `agent_card_url` field in the identity record (identity.md Section 4.3) is present only for agents.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Agent Card** | A JSON document published at an HTTPS URL that describes an agent's capabilities, skills, tools, and protocol preferences. |
| **Capability** | A specific protocol feature that an agent supports, such as streaming responses or push notifications. Declared in the `capabilities` object. |
| **Skill** | A named, described function or area of expertise that an agent can perform. Declared in the `skills` array. Skills are human-readable descriptions, not executable code. |
| **Well-known endpoint** | The conventional URL path for an Agent Card, following the pattern `https://{provider}/.well-known/ainm/{name}/agent-card.json`. The actual URL is stored in the identity record's `agent_card_url` field. |
| **Content type support** | The set of content block types (from content.md) that an agent can produce in its messages. Declared in the `ainm.contentTypes` array. |
| **AINM extension object** | The top-level `ainm` key in the Agent Card JSON that contains all AINM-specific fields. Namespaced to avoid breaking A2A validators. |
| **Tool** | A discrete function an agent can invoke during message processing. Exposed in the `ainm.tools` array with name, description, and JSON Schema input specification. |

## 3. Discovery

Agent Cards are discovered through the identity resolution mechanism defined in identity.md. The discovery flow is:

### 3.1 Resolution Flow

1. **Resolve identity** -- Given an agent's address (e.g., `code-bot@agents.example.com`), resolve the identity record via the well-known endpoint: `GET https://agents.example.com/.well-known/ainm/code-bot` (identity.md Section 4.1).

2. **Read `agent_card_url`** -- The identity record for agents includes an `agent_card_url` field (identity.md Section 4.3). This field is OPTIONAL on the identity record and is present only when `type` is `"agent"`. If the field is absent or the identity `type` is `"human"`, there is no Agent Card.

3. **Fetch the Agent Card** -- Issue an HTTP GET request to the `agent_card_url`. The response is a JSON document conforming to the schema defined in Section 5.

```
GET https://agents.example.com/.well-known/ainm/code-bot/agent-card.json HTTP/1.1
Host: agents.example.com
Accept: application/json
```

### 3.2 URL Convention

The Agent Card URL SHOULD follow the convention:

```
https://{provider}/.well-known/ainm/{name}/agent-card.json
```

However, the `agent_card_url` in the identity record MAY be any valid HTTPS URL. Clients MUST NOT assume the URL follows the conventional pattern -- they MUST use the exact URL from the identity record.

### 3.3 Transport Requirements

- The Agent Card URL MUST use the `https` scheme. Clients MUST reject URLs that use `http` or any other scheme.
- The response Content-Type MUST be `application/json`.
- Servers MUST return the complete Agent Card JSON document on a successful GET request with HTTP status 200.

### 3.4 Discovery Sequence

```
Client                        Provider                      Agent Card Host
  |                              |                              |
  |-- GET /.well-known/ainm/{n} ->                              |
  |                              |                              |
  |<-- 200 Identity Record ------|                              |
  |   { agent_card_url: "..." }  |                              |
  |                              |                              |
  |-- GET {agent_card_url} ---------------------------->        |
  |                                                     |       |
  |<-- 200 Agent Card JSON -----------------------------|       |
  |   { name, capabilities, ainm: {...} }                       |
```

Note: The identity record's provider and the Agent Card host MAY be different domains. The `agent_card_url` is an opaque URL -- the Agent Card may be hosted on a CDN, a separate documentation server, or any HTTPS endpoint.

## 4. Caching

Agent Cards use standard HTTP caching with `ETag` and `If-None-Match` headers (per D-10). This avoids unnecessary data transfer and reduces latency for repeated capability lookups.

### 4.1 Server Requirements

Servers MUST include an `ETag` header on successful Agent Card responses (HTTP 200). The ETag value MUST change whenever the Agent Card content changes. Servers SHOULD use a strong ETag (not weak).

```
HTTP/1.1 200 OK
Content-Type: application/json
ETag: "v1.2.0-a3f8b2c1"
Cache-Control: public, max-age=3600
```

### 4.2 Client Requirements

Clients SHOULD cache Agent Cards locally and use conditional requests to check for updates. When making a conditional request, the client includes the `If-None-Match` header with the previously received ETag value:

```
GET https://agents.example.com/.well-known/ainm/code-bot/agent-card.json HTTP/1.1
Host: agents.example.com
Accept: application/json
If-None-Match: "v1.2.0-a3f8b2c1"
```

If the Agent Card has not changed, the server responds with:

```
HTTP/1.1 304 Not Modified
ETag: "v1.2.0-a3f8b2c1"
```

The client uses its cached copy. No body is transmitted.

### 4.3 Version Field

In addition to HTTP ETag caching, the Agent Card contains a `version` field (Section 5) that provides an application-level version signal. Clients MAY use the `version` field to detect logical changes between cached copies or to display the card version in the UI.

The `version` field and the ETag are independent: the ETag is an opaque HTTP caching token; the `version` is a human-readable semver string managed by the agent operator.

### 4.4 Cache Invalidation

Clients SHOULD refresh their cached Agent Card when:

- The cached ETag is stale (server returns 200 with a new ETag instead of 304).
- The agent's behavior does not match the cached capabilities (e.g., the agent sends a content type not declared in `ainm.contentTypes`).
- The client detects that the `version` field has changed (from a new fetch).

## 5. A2A-Compatible Schema

The top-level fields of an AINM Agent Card match the A2A Agent Card specification. These fields MUST be present and valid so that A2A-compatible clients can read AINM Agent Cards.

### 5.1 Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | REQUIRED | Agent display name. Human-readable, suitable for UI display. Example: `"Code Assistant"`. |
| `description` | string | REQUIRED | Agent description. A brief summary of what the agent does, intended for display in agent directories and profile views. |
| `url` | string | REQUIRED | Agent's primary URL. For AINM agents, this SHOULD be the agent's identity resolution URL (e.g., `https://agents.example.com/.well-known/ainm/code-bot`). |
| `version` | string | REQUIRED | Agent Card version in semver format (e.g., `"1.2.0"`). Incremented by the agent operator when the card content changes. |
| `protocolVersion` | string | REQUIRED | A2A protocol version for A2A client compatibility. MUST be a valid A2A version string (e.g., `"0.2.1"`). This signals to A2A clients which version of the A2A protocol the top-level fields conform to. |
| `capabilities` | object | REQUIRED | A2A capabilities object declaring high-level agent features. See Section 5.2. |
| `skills` | array | REQUIRED | Array of skill objects describing what the agent can do. See Section 5.3. |
| `defaultInputModes` | array | OPTIONAL | Array of MIME type strings the agent accepts as input. Example: `["text/plain", "application/json"]`. If omitted, defaults to `["text/plain"]`. |
| `defaultOutputModes` | array | OPTIONAL | Array of MIME type strings the agent produces as output. Example: `["text/plain", "application/json"]`. If omitted, defaults to `["text/plain"]`. |

### 5.2 Capabilities Object

The `capabilities` object declares high-level agent features in A2A-compatible format:

```json
{
  "capabilities": {
    "streaming": true,
    "pushNotifications": false
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `streaming` | boolean | REQUIRED | Whether the agent supports streaming responses. A2A clients use this to determine if they can receive incremental output. |
| `pushNotifications` | boolean | REQUIRED | Whether the agent supports push notifications for asynchronous task updates. |

### 5.3 Skills Array

Each skill object in the `skills` array describes a named capability:

```json
{
  "skills": [
    {
      "id": "code-review",
      "name": "Code Review",
      "description": "Reviews code for bugs, performance issues, and style violations. Supports Python, JavaScript, TypeScript, Rust, and Swift.",
      "tags": ["code", "review", "quality"]
    },
    {
      "id": "code-generation",
      "name": "Code Generation",
      "description": "Generates code from natural language descriptions. Can produce complete functions, classes, or modules.",
      "tags": ["code", "generation", "ai"]
    }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | REQUIRED | Unique identifier for the skill within this Agent Card. Alphanumeric with hyphens (e.g., `"code-review"`). |
| `name` | string | REQUIRED | Human-readable skill name. |
| `description` | string | REQUIRED | Detailed description of what the skill does, including any constraints or supported inputs. |
| `tags` | array | OPTIONAL | Array of string tags for categorization and search. |

## 6. AINM Extension Object

All AINM-specific fields live under a top-level `ainm` object. This namespacing ensures that A2A validators (which may reject unknown top-level fields) skip the AINM extensions, while AINM clients can access rich capability information.

### 6.1 Extension Object Structure

```json
{
  "ainm": {
    "protocolVersion": "0.1.0",
    "contentTypes": ["text", "code", "image", "file", "button", "form", "table", "approval"],
    "streaming": {
      "supported": true,
      "parallelBlocks": true,
      "preferredChunkSize": "token"
    },
    "threading": {
      "subThreads": true,
      "maxConcurrentThreads": 5
    },
    "memory": {
      "archivalSupported": true,
      "contextFramePreferred": true
    },
    "tools": [
      {
        "name": "code_search",
        "description": "Search codebase for patterns and definitions",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": { "type": "string", "description": "Search query" },
            "scope": { "type": "string", "description": "Directory scope" }
          },
          "required": ["query"]
        }
      }
    ]
  }
}
```

### 6.2 Field Definitions

**`ainm.protocolVersion`** (string, REQUIRED)

The AINM protocol version this agent supports, in semver format (e.g., `"0.1.0"`). Distinct from the top-level `protocolVersion` (which is the A2A version) and `version` (which is the card's own version).

**`ainm.contentTypes`** (array of strings, REQUIRED)

Content block types the agent can produce in its messages. Values are drawn from the core set defined in content.md and the interactive set defined in interactive.md:

- **Core types** (content.md): `"text"`, `"code"`, `"image"`, `"file"`
- **Interactive types** (interactive.md): `"button"`, `"form"`, `"table"`, `"approval"`
- **Extension types**: Any reverse-domain-namespaced type (e.g., `"com.example.weather-card"`)

Clients use this field to understand what kinds of content blocks to expect from the agent. An agent MUST NOT send content block types that are not declared in its `ainm.contentTypes` array.

**`ainm.streaming`** (object, REQUIRED)

Streaming capability details for AINM clients:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `supported` | boolean | REQUIRED | Whether the agent supports AINM streaming (stream-start/delta/end lifecycle). |
| `parallelBlocks` | boolean | REQUIRED | Whether the agent can stream into multiple content blocks simultaneously (D-03). If `false`, the agent streams one block at a time. |
| `preferredChunkSize` | string | REQUIRED | The agent's preferred delta granularity. One of: `"token"` (1-few tokens per delta, lowest latency) or `"sentence"` (sentence-level chunks, fewer events). Clients MAY use this hint to adjust rendering behavior. |

**`ainm.threading`** (object, REQUIRED)

Threading capability details:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `subThreads` | boolean | REQUIRED | Whether the agent supports participating in sub-threads (threading.md Section 4). |
| `maxConcurrentThreads` | integer or null | REQUIRED | Maximum number of threads the agent can actively participate in simultaneously. `null` means unlimited. Clients and servers MAY use this to throttle thread invitations. |

**`ainm.memory`** (object, REQUIRED)

Memory capability details:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `archivalSupported` | boolean | REQUIRED | Whether the agent supports archival memory operations (storing and querying cross-conversation knowledge). |
| `contextFramePreferred` | boolean | REQUIRED | Whether the agent prefers to receive a context frame on thread join rather than raw message history. When `true`, the server SHOULD deliver a structured context frame per D-14. |

**`ainm.tools`** (array, REQUIRED)

Array of tool objects describing the tools the agent can invoke. Each tool follows the MCP tool listing format (per D-12):

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | REQUIRED | Tool name. MUST be unique within the `ainm.tools` array. Alphanumeric with underscores and hyphens (e.g., `"code_search"`, `"file-read"`). |
| `description` | string | REQUIRED | Human-readable description of what the tool does. |
| `inputSchema` | object | REQUIRED | JSON Schema (Draft 2020-12) describing the tool's input parameters. MUST be a valid JSON Schema object with `type: "object"`. |

If the agent has no tools, this field MUST be an empty array (`[]`).

The `ainm.tools` array exposes only the tool name, description, and input schema. **Output schemas and implementation details are not disclosed.** This is a deliberate security boundary -- the Agent Card reveals what the tool accepts, not what it returns or how it works internally.

## 7. Agent Card Validation

### 7.1 Required Field Validation

Clients MUST validate that all REQUIRED top-level fields are present and have the correct types:

- `name` is a non-empty string
- `description` is a non-empty string
- `url` is a valid HTTPS URL
- `version` is a non-empty string
- `protocolVersion` is a non-empty string
- `capabilities` is an object with `streaming` (boolean) and `pushNotifications` (boolean)
- `skills` is an array

### 7.2 AINM Extension Validation

Clients SHOULD validate that the `ainm` object is present for AINM-specific features. If the `ainm` object is absent, the client SHOULD treat the card as an A2A-only Agent Card and disable AINM-specific features (e.g., AINM streaming, tool visibility, memory integration).

When the `ainm` object is present, clients SHOULD validate:

- `ainm.protocolVersion` is a non-empty string
- `ainm.contentTypes` is a non-empty array of strings
- `ainm.streaming` is an object with `supported`, `parallelBlocks`, and `preferredChunkSize`
- `ainm.threading` is an object with `subThreads` and `maxConcurrentThreads`
- `ainm.memory` is an object with `archivalSupported` and `contextFramePreferred`
- `ainm.tools` is an array (may be empty)

### 7.3 Forward Compatibility

Unknown top-level fields MUST be ignored. This allows the A2A specification to evolve without breaking AINM clients.

Unknown fields within the `ainm` object MUST be ignored. This allows future AINM protocol versions to add new extension fields without breaking existing AINM clients.

Clients MUST NOT reject an Agent Card because it contains fields they do not recognize.

## 8. Error Handling

Agent Card errors use JSON-RPC error codes in the range **-32060 to -32069**.

### 8.1 Error Codes

| Code | Name | Description |
|------|------|-------------|
| `-32060` | Agent Card Not Found | The `agent_card_url` returned HTTP 404. The agent does not have a published Agent Card, or the URL is stale. |
| `-32061` | Invalid Agent Card Format | The response body is not valid JSON, or the JSON is missing required fields. The Agent Card cannot be parsed. |
| `-32062` | Agent Card Version Mismatch | The Agent Card's `ainm.protocolVersion` is incompatible with the client's supported protocol version range. The client cannot process this card. |

### 8.2 Error Response Format

Errors are reported using the standard JSON-RPC error format:

```json
{
  "error": {
    "code": -32060,
    "message": "Agent Card not found at the specified URL",
    "data": {
      "agent_card_url": "https://agents.example.com/.well-known/ainm/code-bot/agent-card.json",
      "http_status": 404
    }
  }
}
```

```json
{
  "error": {
    "code": -32061,
    "message": "Invalid Agent Card: missing required field 'capabilities'",
    "data": {
      "missing_fields": ["capabilities"]
    }
  }
}
```

```json
{
  "error": {
    "code": -32062,
    "message": "Agent Card protocol version '2.0.0' is not supported by this client",
    "data": {
      "card_version": "2.0.0",
      "supported_range": ">=0.1.0 <1.0.0"
    }
  }
}
```

### 8.3 HTTP Error Mapping

When fetching an Agent Card via HTTP, clients SHOULD map HTTP error statuses to protocol errors:

| HTTP Status | Protocol Error Code | Behavior |
|-------------|---------------------|----------|
| 200 | (success) | Parse and validate the Agent Card. |
| 304 | (success) | Use cached Agent Card. |
| 404 | -32060 | Agent Card not found. |
| 5xx | -32060 | Server error; treat as temporarily unavailable. Client SHOULD retry with exponential backoff. |
| Other 4xx | -32061 | Unexpected client error. Log for debugging. |

## 9. Security Considerations

### 9.1 Public Documents

Agent Cards are **public documents**. They are served over HTTPS without authentication and are intended to be freely accessible. Agent Cards MUST NOT contain:

- Private keys or key material
- Authentication tokens, API keys, or secrets
- Internal server addresses or infrastructure details
- Personally identifiable information beyond the agent's public-facing name and description

### 9.2 Integrity Through Identity Resolution

The `agent_card_url` in the identity record is integrity-protected by the identity resolution mechanism. The identity record is served over TLS from the agent's provider (identity.md Section 4.1), ensuring that the URL has not been tampered with in transit. An attacker who cannot compromise the provider's TLS cannot redirect clients to a forged Agent Card.

### 9.3 HTTPS Requirement

Agent Cards MUST be served over HTTPS. Clients MUST reject Agent Card URLs that use `http` or any other non-HTTPS scheme. This ensures confidentiality and integrity of the Agent Card content during transit.

### 9.4 Tool Input Schema Exposure

Tools listed in `ainm.tools` expose only the input schema -- the JSON Schema describing what parameters the tool accepts. Output schemas, implementation details, internal endpoints, and any information about what the tool returns are NOT disclosed. This is a deliberate information boundary:

- **Exposed**: Tool name, description, input parameters (types, descriptions, constraints)
- **Not exposed**: Return value schema, internal API endpoints, execution environment, rate limits, cost information

Agents that wish to share output schema information MAY do so through their skill descriptions or documentation URL, but the Agent Card itself does not carry output schemas.

### 9.5 Agent Card Impersonation

A malicious actor could publish a fake Agent Card at a URL they control. This is mitigated by the identity resolution chain: the `agent_card_url` comes from the identity record, which is served by the agent's provider over TLS. To impersonate an agent's capabilities, an attacker would need to compromise the provider's DNS or TLS certificate -- the same attack surface as email spoofing.

## 10. Examples

### 10.1 Full Agent Card (All Fields)

A fully-specified AINM Agent Card for a code assistant:

```json
{
  "name": "Code Assistant",
  "description": "An AI-powered code assistant that reviews, generates, and explains code across multiple programming languages. Supports Python, JavaScript, TypeScript, Rust, and Swift.",
  "url": "https://agents.example.com/.well-known/ainm/code-bot",
  "version": "1.2.0",
  "protocolVersion": "0.2.1",
  "capabilities": {
    "streaming": true,
    "pushNotifications": false
  },
  "skills": [
    {
      "id": "code-review",
      "name": "Code Review",
      "description": "Reviews code for bugs, performance issues, security vulnerabilities, and style violations. Provides actionable suggestions with code examples.",
      "tags": ["code", "review", "quality", "security"]
    },
    {
      "id": "code-generation",
      "name": "Code Generation",
      "description": "Generates code from natural language descriptions. Can produce complete functions, classes, modules, or project scaffolds.",
      "tags": ["code", "generation", "ai"]
    },
    {
      "id": "code-explanation",
      "name": "Code Explanation",
      "description": "Explains existing code in plain language, including control flow, design patterns, and potential issues.",
      "tags": ["code", "explanation", "documentation"]
    }
  ],
  "defaultInputModes": ["text/plain", "application/json"],
  "defaultOutputModes": ["text/plain", "application/json"],
  "ainm": {
    "protocolVersion": "0.1.0",
    "contentTypes": ["text", "code", "image", "file", "button", "form", "table"],
    "streaming": {
      "supported": true,
      "parallelBlocks": true,
      "preferredChunkSize": "token"
    },
    "threading": {
      "subThreads": true,
      "maxConcurrentThreads": 10
    },
    "memory": {
      "archivalSupported": true,
      "contextFramePreferred": true
    },
    "tools": [
      {
        "name": "code_search",
        "description": "Search codebase for patterns, function definitions, and references",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": {
              "type": "string",
              "description": "Search query -- regex patterns, function names, or natural language"
            },
            "scope": {
              "type": "string",
              "description": "Directory scope to limit the search (e.g., 'src/', 'tests/')"
            },
            "file_types": {
              "type": "array",
              "items": { "type": "string" },
              "description": "File extensions to include (e.g., ['.py', '.ts'])"
            }
          },
          "required": ["query"]
        }
      },
      {
        "name": "file_read",
        "description": "Read the contents of a file from the project",
        "inputSchema": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Relative file path from the project root"
            },
            "line_start": {
              "type": "integer",
              "description": "Starting line number (1-based, inclusive)"
            },
            "line_end": {
              "type": "integer",
              "description": "Ending line number (1-based, inclusive)"
            }
          },
          "required": ["path"]
        }
      },
      {
        "name": "run_tests",
        "description": "Execute test suite and return results",
        "inputSchema": {
          "type": "object",
          "properties": {
            "test_path": {
              "type": "string",
              "description": "Path to specific test file or directory"
            },
            "filter": {
              "type": "string",
              "description": "Test name filter pattern"
            }
          },
          "required": []
        }
      }
    ]
  }
}
```

### 10.2 Minimal Agent Card (A2A-Only Fields)

A minimal Agent Card that satisfies A2A compatibility but does not include AINM extensions:

```json
{
  "name": "Simple Bot",
  "description": "A basic chatbot that answers questions.",
  "url": "https://bots.example.com/.well-known/ainm/simple-bot",
  "version": "1.0.0",
  "protocolVersion": "0.2.1",
  "capabilities": {
    "streaming": false,
    "pushNotifications": false
  },
  "skills": [
    {
      "id": "qa",
      "name": "Question Answering",
      "description": "Answers general knowledge questions based on training data.",
      "tags": ["qa", "general"]
    }
  ]
}
```

An AINM client receiving this card SHOULD recognize the absence of the `ainm` object and treat the agent as an A2A-only agent, disabling AINM-specific features.

### 10.3 Agent Card for a Tool-Heavy Agent

An agent with many tools and minimal conversational skills:

```json
{
  "name": "Data Pipeline Manager",
  "description": "Manages data pipeline operations -- scheduling, monitoring, and debugging ETL jobs across production and staging environments.",
  "url": "https://infra.example.com/.well-known/ainm/pipeline-mgr",
  "version": "2.0.1",
  "protocolVersion": "0.2.1",
  "capabilities": {
    "streaming": true,
    "pushNotifications": true
  },
  "skills": [
    {
      "id": "pipeline-monitoring",
      "name": "Pipeline Monitoring",
      "description": "Monitors running pipelines and reports status, errors, and performance metrics.",
      "tags": ["data", "monitoring", "etl"]
    },
    {
      "id": "job-management",
      "name": "Job Management",
      "description": "Schedules, pauses, resumes, and cancels ETL jobs. Supports cron-like scheduling.",
      "tags": ["data", "scheduling", "etl"]
    }
  ],
  "defaultInputModes": ["text/plain", "application/json"],
  "defaultOutputModes": ["text/plain", "application/json"],
  "ainm": {
    "protocolVersion": "0.1.0",
    "contentTypes": ["text", "code", "table", "button", "approval"],
    "streaming": {
      "supported": true,
      "parallelBlocks": false,
      "preferredChunkSize": "sentence"
    },
    "threading": {
      "subThreads": false,
      "maxConcurrentThreads": 3
    },
    "memory": {
      "archivalSupported": false,
      "contextFramePreferred": true
    },
    "tools": [
      {
        "name": "list_pipelines",
        "description": "List all data pipelines with their current status",
        "inputSchema": {
          "type": "object",
          "properties": {
            "environment": {
              "type": "string",
              "enum": ["production", "staging", "development"],
              "description": "Target environment"
            },
            "status_filter": {
              "type": "string",
              "enum": ["running", "failed", "paused", "all"],
              "description": "Filter by pipeline status"
            }
          },
          "required": ["environment"]
        }
      },
      {
        "name": "get_pipeline_logs",
        "description": "Retrieve recent logs for a specific pipeline run",
        "inputSchema": {
          "type": "object",
          "properties": {
            "pipeline_id": {
              "type": "string",
              "description": "Pipeline identifier"
            },
            "run_id": {
              "type": "string",
              "description": "Specific run identifier (latest if omitted)"
            },
            "tail": {
              "type": "integer",
              "description": "Number of most recent log lines to return",
              "default": 100
            }
          },
          "required": ["pipeline_id"]
        }
      },
      {
        "name": "trigger_pipeline",
        "description": "Trigger a manual pipeline run",
        "inputSchema": {
          "type": "object",
          "properties": {
            "pipeline_id": {
              "type": "string",
              "description": "Pipeline identifier"
            },
            "environment": {
              "type": "string",
              "enum": ["production", "staging", "development"],
              "description": "Target environment"
            },
            "parameters": {
              "type": "object",
              "description": "Pipeline-specific parameters as key-value pairs"
            }
          },
          "required": ["pipeline_id", "environment"]
        }
      },
      {
        "name": "pause_pipeline",
        "description": "Pause a running pipeline. Can be resumed later.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "pipeline_id": {
              "type": "string",
              "description": "Pipeline identifier"
            },
            "reason": {
              "type": "string",
              "description": "Reason for pausing (logged for audit)"
            }
          },
          "required": ["pipeline_id"]
        }
      },
      {
        "name": "get_metrics",
        "description": "Get performance metrics for a pipeline over a time range",
        "inputSchema": {
          "type": "object",
          "properties": {
            "pipeline_id": {
              "type": "string",
              "description": "Pipeline identifier"
            },
            "metric": {
              "type": "string",
              "enum": ["throughput", "latency", "error_rate", "cost"],
              "description": "Metric to retrieve"
            },
            "period": {
              "type": "string",
              "enum": ["1h", "6h", "24h", "7d", "30d"],
              "description": "Time period for aggregation"
            }
          },
          "required": ["pipeline_id", "metric"]
        }
      }
    ]
  }
}
```

## References

- **identity.md** -- Identity resolution and the `agent_card_url` field (Section 4.2, 4.3)
- **content.md** -- Content block types referenced in `ainm.contentTypes`
- **threading.md** -- Threading model referenced in `ainm.threading`
- **security.md** -- Message signing (Agent Card URLs are integrity-protected through identity resolution)
- **transport.md** -- JSON-RPC methods and WebSocket binding
- **A2A Agent Card Specification** (Google) -- Top-level field compatibility
- **MCP Tool Listing Format** -- `ainm.tools` array aligns with MCP tool definitions
- **RFC 7232** -- HTTP Conditional Requests (ETag, If-None-Match)
- **JSON Schema Draft 2020-12** -- Used for `ainm.tools[].inputSchema`

---

*AI-Native Messaging Protocol -- Agent Card Specification v0.1.0*
*Cross-references: identity.md (discovery), content.md (content types), threading.md (threading capabilities), security.md (integrity), transport.md (methods)*
