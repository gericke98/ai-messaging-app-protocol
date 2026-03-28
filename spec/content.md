# Content Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Content blocks are the atomic units of message content in the AI-Native Messaging Protocol. Every message in AINM carries an ordered array of typed content blocks, enabling rich, multi-part messages that go far beyond plain text. A single message can contain explanatory text, a code snippet, an image, and a file attachment -- each as a distinct, independently typed block.

The content system is built on two principles: **structured typing** and **graceful degradation**. Structured typing means every block declares its type and version, allowing clients to render content appropriately and agents to process messages programmatically. Graceful degradation means every block carries a mandatory fallback representation, so clients that do not understand a particular block type or version can still display meaningful content.

Extensibility follows a closed-core-plus-open-extensions model. The protocol defines four core block types (text, code, image, file) that all implementations MUST support. Third parties can define additional block types using reverse-domain namespacing (e.g., `com.example.weather-card`). Clients render extension types when they recognize them and fall back to the `fallback` field when they do not. This design ensures the protocol can evolve without breaking existing clients.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Content block** | A single unit of typed content within a message. Content blocks are the building blocks of message payloads. |
| **Block type** | A string identifier that determines how a content block is structured and rendered. Core types are bare strings (e.g., `"text"`); extension types use reverse-domain namespacing (e.g., `"com.example.weather-card"`). |
| **Core block** | A content block whose type is defined by this specification. Implementations MUST be able to render all core block types. |
| **Extension block** | A content block whose type is defined by a third party using reverse-domain namespacing. Implementations MAY render extension block types. |
| **Fallback** | A plain text or Markdown representation of a content block's content. Every content block carries a fallback that clients render when they do not recognize the block type or version. |
| **Source** | A trust annotation on a content block indicating who produced the content: `"user"` (human input), `"agent"` (AI-generated), or `"system"` (protocol/server-generated). See security.md for full semantics. |
| **Version** | A `MAJOR.MINOR` string on each content block indicating the schema version of that block type. Clients use the version to determine which fields to expect. |

## 3. Content Block Base Schema

Every content block shares these base fields. Implementations MUST include all four fields on every content block.

```json
{
  "type": "<block-type-identifier>",
  "version": "1.0",
  "source": "user",
  "fallback": "<plain-text-or-markdown-fallback>"
}
```

### Field Definitions

**`type`** (string, REQUIRED)

Block type identifier. Core types are bare strings: `"text"`, `"code"`, `"image"`, `"file"`. Extension types use reverse-domain namespacing: `"com.example.weather-card"`, `"io.mycompany.approval-form"`. See Section 5 for extension type rules.

**`version`** (string, REQUIRED)

Semantic version of the block type schema in `MAJOR.MINOR` format. The version tracks the evolution of a specific block type's schema, independent of the protocol version. Version semantics:

- **MAJOR** increments indicate breaking changes -- fields removed, field types changed, or required fields added. Clients that do not support the new MAJOR version MUST render the `fallback` field instead.
- **MINOR** increments indicate backward-compatible additions -- new optional fields added. Clients supporting an older MINOR version of the same MAJOR can safely ignore unrecognized optional fields.

Example: a client supporting `"text"` version `1.0` receives a `"text"` block at version `1.2`. Since the MAJOR version matches, the client renders the block normally, ignoring any optional fields introduced in versions `1.1` and `1.2`. If the client receives a `"text"` block at version `2.0`, it renders the `fallback` field.

**`source`** (string, REQUIRED)

Trust annotation indicating who produced this content block. One of:

- `"user"` -- Content produced by a human participant. Set when a human types text, uploads a file, or shares an image.
- `"agent"` -- Content produced by an AI agent. Set when an agent generates text, code, or other content.
- `"system"` -- Content produced by the protocol infrastructure or server. Used for system notifications, thread events, and server-generated metadata.

The `source` field is set by the message sender and verified through the message signature (see security.md). Agents and clients can use the `source` field to distinguish human input from AI-generated content, which is essential for preventing prompt injection attacks. See security.md for detailed trust model semantics.

**`fallback`** (string, REQUIRED)

Plain text or Markdown representation of the block's content. Implementations MUST render the `fallback` field when:

1. The block `type` is not recognized (unknown extension type).
2. The block `version` MAJOR is higher than the implementation supports.
3. The implementation cannot render the block for any other reason (missing renderer, corrupt data).

The `fallback` field guarantees that every content block degrades gracefully to readable text, regardless of client capabilities.

## 4. Core Block Types

The protocol defines four core block types. Implementations MUST be able to render all four.

### 4.1 TextBlock

The most common block type. Contains Markdown-formatted text with optional mention annotations.

```json
{
  "type": "text",
  "version": "1.0",
  "source": "user",
  "text": "Hello, can you help me with this code?\n\nI'm getting a **weird error** on line 42.",
  "mentions": [
    {
      "address": "code-bot@agents.example.com",
      "offset": 4,
      "length": 33
    }
  ],
  "fallback": "Hello, can you help me with this code? I'm getting a weird error on line 42."
}
```

**Fields:**

- **`text`** (string, REQUIRED): Markdown-formatted text content. Supports CommonMark syntax for formatting (bold, italic, links, lists, etc.). Raw text -- no HTML allowed.
- **`mentions`** (array, OPTIONAL): Array of mention annotations referencing participant addresses. Each mention object contains:
  - **`address`** (string, REQUIRED): Full AINM address of the mentioned participant (e.g., `"code-bot@agents.example.com"`).
  - **`offset`** (integer, REQUIRED): Zero-based character offset in the `text` field where the mention display text begins.
  - **`length`** (integer, REQUIRED): Length of the mention display text in characters.

Mentions enable @-mention functionality that works uniformly across participant types -- human and agent addresses are referenced the same way (MIXD-04). The mention annotation is separate from the text content: the display text appears inline in `text`, and the `mentions` array provides the semantic link to participant addresses. Mention rendering behavior is normatively defined in threading.md.

### 4.2 CodeBlock

Contains source code with language identification and optional filename.

```json
{
  "type": "code",
  "version": "1.0",
  "source": "agent",
  "language": "python",
  "code": "def fibonacci(n):\n    if n <= 1:\n        return n\n    return fibonacci(n-1) + fibonacci(n-2)",
  "filename": "fibonacci.py",
  "fallback": "```python\ndef fibonacci(n):\n    if n <= 1:\n        return n\n    return fibonacci(n-1) + fibonacci(n-2)\n```"
}
```

**Fields:**

- **`language`** (string, REQUIRED): Programming language identifier. Use common identifiers: `python`, `javascript`, `typescript`, `rust`, `swift`, `json`, `yaml`, `markdown`, `bash`, `sql`, `html`, `css`, `go`, `java`, `kotlin`, `c`, `cpp`, `csharp`, `ruby`, `php`, `shell`, `plaintext`. This field is used by clients for syntax highlighting.
- **`code`** (string, REQUIRED): Source code content. Raw code -- no Markdown wrapping, no code fences. Whitespace and newlines are preserved as-is.
- **`filename`** (string, OPTIONAL): Suggested filename for the code. Informational only -- used for display and download suggestions.

### 4.3 ImageBlock

References an image resource with metadata for display.

```json
{
  "type": "image",
  "version": "1.0",
  "source": "user",
  "url": "https://cdn.example.com/images/screenshot.png",
  "mime_type": "image/png",
  "alt": "Screenshot showing the error dialog",
  "width": 1920,
  "height": 1080,
  "size_bytes": 245760,
  "fallback": "[Image: Screenshot showing the error dialog]"
}
```

**Fields:**

- **`url`** (string, REQUIRED): HTTPS URL to the image resource. Implementations MUST reject URLs that do not use the `https` scheme. HTTP (unencrypted) URLs are not permitted.
- **`mime_type`** (string, REQUIRED): IANA media type of the image. Supported types: `image/png`, `image/jpeg`, `image/webp`, `image/gif`, `image/svg+xml`. Implementations MUST support at least `image/png` and `image/jpeg`.
- **`alt`** (string, REQUIRED): Alternative text description for accessibility. Describes the image content for screen readers and text-only rendering. This field is distinct from `fallback` -- `alt` describes what the image shows, while `fallback` provides the text displayed when the block type is unrecognized.
- **`width`** (integer, OPTIONAL): Image width in pixels. Used by clients for layout before the image loads.
- **`height`** (integer, OPTIONAL): Image height in pixels. Used by clients for layout before the image loads.
- **`size_bytes`** (integer, OPTIONAL): File size in bytes. Used by clients to warn before downloading large images.

### 4.4 FileBlock

References a file resource with integrity verification metadata.

```json
{
  "type": "file",
  "version": "1.0",
  "source": "user",
  "url": "https://cdn.example.com/files/report.pdf",
  "mime_type": "application/pdf",
  "filename": "quarterly-report.pdf",
  "size_bytes": 1048576,
  "digest": "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
  "fallback": "[File: quarterly-report.pdf (1.0 MB, PDF)]"
}
```

**Fields:**

- **`url`** (string, REQUIRED): HTTPS URL to the file resource. Implementations MUST reject URLs that do not use the `https` scheme.
- **`mime_type`** (string, REQUIRED): IANA media type of the file (e.g., `application/pdf`, `text/csv`, `application/zip`).
- **`filename`** (string, REQUIRED): Original filename for display and download. This is the name presented to the user when downloading the file.
- **`size_bytes`** (integer, REQUIRED): File size in bytes. Clients use this to display file size and to warn before downloading large files.
- **`digest`** (string, REQUIRED): Content hash for integrity verification. Format: `algorithm:hex-encoded-hash`. The algorithm MUST be `sha256`. Clients SHOULD verify the downloaded file's SHA-256 hash matches the `digest` value. This enables integrity verification independent of transport security -- the file content can be verified even if it passes through untrusted intermediate storage.

## 5. Extension Block Types

Extension block types allow third parties to define custom content blocks without modifying the protocol. Extensions use reverse-domain namespacing to avoid collisions between different implementers.

### Naming Convention

Extension type identifiers use reverse-domain format: `{reversed-domain}.{block-name}`. Examples:

- `com.example.weather-card`
- `io.mycompany.approval-form`
- `org.openai.function-call`

The namespace SHOULD correspond to a domain controlled by the extension author. This protocol does not define a namespace registry. Collisions between different implementers using the same namespace are resolved by the client's extension rendering logic.

### Rendering Rules

- Clients MUST render core block types (`text`, `code`, `image`, `file`).
- Clients MAY render extension block types if they recognize the type and version.
- Clients that do not recognize an extension block type MUST render the `fallback` field.
- Clients that recognize an extension type but not its MAJOR version MUST render the `fallback` field.

### Extension Block Structure

Extension blocks include the base schema fields (`type`, `version`, `source`, `fallback`) plus any additional fields defined by the extension. There are no restrictions on additional field names or types beyond valid JSON.

```json
{
  "type": "com.example.weather-card",
  "version": "1.0",
  "source": "agent",
  "data": {
    "city": "Madrid",
    "temperature": 22,
    "unit": "celsius",
    "condition": "sunny"
  },
  "fallback": "Madrid: 22C, Sunny"
}
```

In this example, a client that recognizes `com.example.weather-card` renders a rich weather card UI. A client that does not recognize the type renders: "Madrid: 22C, Sunny".

### Discovery and Documentation

Extension authors SHOULD publish documentation for their extension block types, including:

- The extension type identifier.
- The version and any additional fields beyond the base schema.
- Rendering guidance for clients that choose to support the extension.

This protocol does not define a mechanism for automatic extension discovery or a centralized registry. Extension support is negotiated out-of-band between implementers.

## 6. Content Block Ordering

- Content blocks are ordered: the `content` array in a message envelope preserves insertion order.
- Clients SHOULD render blocks in array order (top to bottom).
- A single message MAY contain multiple blocks of different types (e.g., a text explanation followed by a code block followed by another text block).
- A single message MAY contain multiple blocks of the same type (e.g., two text blocks).
- There is no protocol-imposed limit on the number of content blocks per message. Servers MAY communicate a `maxContentBlocks` limit during capability exchange (see transport.md).

## 7. Error Handling

### Missing Required Fields

Implementations MUST reject content blocks with missing required fields. When a content block in a received message is missing a required field (`type`, `version`, `source`, `fallback`, or any type-specific required field), the receiving implementation returns a JSON-RPC error with code `-32013` (Malformed message) via the transport layer.

### Unknown Block Type Version

If a content block's `version` field has a MAJOR version higher than the implementation supports for that block type, the implementation MUST render the `fallback` field. This applies to both core and extension block types.

If the `version` field has the same MAJOR but a higher MINOR version, the implementation renders the block normally, ignoring any unrecognized optional fields.

### Empty Content Array

The `content` array semantics depend on the message `type` field in the envelope (see security.md for the full message envelope):

- For messages with `type: "message"`, the `content` array MUST contain at least one content block.
- For other message types (e.g., `thread.join`, `thread.leave`, `presence`), the `content` array MAY be empty.

### Invalid URL Scheme

ImageBlock and FileBlock `url` fields MUST use the `https` scheme. Implementations MUST reject content blocks where the `url` field uses `http` or any other non-HTTPS scheme, returning JSON-RPC error `-32013` (Malformed message).

## 8. Security Considerations

### URL Validation

All `url` fields in ImageBlock and FileBlock MUST use HTTPS. This requirement prevents content from being loaded over unencrypted connections, which could expose file contents to network observers or allow content tampering via man-in-the-middle attacks.

### Content Hash Verification

The FileBlock `digest` field enables end-to-end content integrity verification. Clients SHOULD verify the SHA-256 hash of downloaded files against the `digest` value. A mismatch indicates the file has been tampered with or corrupted, and the client SHOULD warn the user and discard the file.

The `digest` is computed over the original file content and included in the signed message envelope. Because the message envelope is signed with Ed25519 (see security.md), the `digest` value is protected by the message signature -- an attacker cannot modify the `digest` without invalidating the signature.

### Fallback Spoofing

The `fallback` field could potentially misrepresent the actual block content. For example, an extension block with a weather card showing 40 degrees could have a fallback that says "22C, Sunny". To mitigate this:

- Clients rendering the actual block type (because they recognize and support it) SHOULD NOT also display the fallback.
- Clients rendering the fallback SHOULD indicate that they are showing a simplified representation (e.g., via visual styling or a label).

### Source Trust Annotations

The `source` field is a trust signal, not a security boundary. It is set by the message sender and verified through the message signature -- if Alice's message contains a block with `"source": "agent"`, recipients know Alice (or Alice's agent) claims the content was AI-generated, and Alice's signature confirms the claim came from Alice. See security.md for the full trust model.

## 9. Examples

### Simple Text-Only Message

A human user sending a plain text message:

```json
{
  "id": "019580a1-7b3e-7000-8000-abcdef123456",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:34:56.789Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "Hey everyone, has anyone tried the new API endpoint?",
      "fallback": "Hey everyone, has anyone tried the new API endpoint?"
    }
  ],
  "signature": "dGhpcyBpcyBhIGJhc2U2NHVybCBlbmNvZGVkIGVkMjU1MTkgc2lnbmF0dXJl..."
}
```

### Rich Agent Response (TextBlock + CodeBlock)

An agent responding with an explanation and code:

```json
{
  "id": "019580a2-9c4f-7000-8000-fedcba987654",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:35:02.123Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Here's a working example of the Fibonacci function with memoization for better performance:",
      "fallback": "Here's a working example of the Fibonacci function with memoization for better performance:"
    },
    {
      "type": "code",
      "version": "1.0",
      "source": "agent",
      "language": "python",
      "code": "from functools import lru_cache\n\n@lru_cache(maxsize=None)\ndef fibonacci(n: int) -> int:\n    if n <= 1:\n        return n\n    return fibonacci(n - 1) + fibonacci(n - 2)\n\n# Usage\nfor i in range(10):\n    print(f\"fib({i}) = {fibonacci(i)}\")",
      "filename": "fibonacci.py",
      "fallback": "```python\nfrom functools import lru_cache\n\n@lru_cache(maxsize=None)\ndef fibonacci(n: int) -> int:\n    if n <= 1:\n        return n\n    return fibonacci(n - 1) + fibonacci(n - 2)\n```"
    }
  ],
  "signature": "YW5vdGhlciBiYXNlNjR1cmwgZW5jb2RlZCBzaWduYXR1cmU..."
}
```

### File Sharing (TextBlock + FileBlock)

A user sharing a document with a description:

```json
{
  "id": "019580a3-b1d2-7000-8000-aabbccddee01",
  "from": "bob@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:36:15.456Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "Here's the quarterly report. @data-analyst@agents.example.com can you summarize the key findings?",
      "mentions": [
        {
          "address": "data-analyst@agents.example.com",
          "offset": 33,
          "length": 37
        }
      ],
      "fallback": "Here's the quarterly report. @data-analyst can you summarize the key findings?"
    },
    {
      "type": "file",
      "version": "1.0",
      "source": "user",
      "url": "https://cdn.example.com/files/q1-2026-report.pdf",
      "mime_type": "application/pdf",
      "filename": "Q1-2026-Report.pdf",
      "size_bytes": 2097152,
      "digest": "sha256:a4b9c3d2e1f0987654321abcdef0123456789fedcba9876543210abcdef012345",
      "fallback": "[File: Q1-2026-Report.pdf (2.0 MB, PDF)]"
    }
  ],
  "signature": "ZmlsZSBzaGFyaW5nIG1lc3NhZ2Ugc2lnbmF0dXJl..."
}
```

### Extension Block with Fallback Rendering

An agent sending a custom weather card that degrades gracefully:

```json
{
  "id": "019580a4-c2e3-7000-8000-112233445566",
  "from": "weather-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:37:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Here's the current weather for Madrid:",
      "fallback": "Here's the current weather for Madrid:"
    },
    {
      "type": "com.example.weather-card",
      "version": "1.0",
      "source": "agent",
      "data": {
        "city": "Madrid",
        "temperature": 22,
        "unit": "celsius",
        "condition": "sunny",
        "humidity": 45,
        "wind_speed_kmh": 12,
        "forecast": [
          { "day": "Tomorrow", "high": 24, "low": 14, "condition": "partly_cloudy" },
          { "day": "Wednesday", "high": 20, "low": 12, "condition": "rain" }
        ]
      },
      "fallback": "Madrid: 22C, Sunny\nHumidity: 45%, Wind: 12 km/h\nTomorrow: 24/14C Partly Cloudy\nWednesday: 20/12C Rain"
    }
  ],
  "signature": "d2VhdGhlciBjYXJkIG1lc3NhZ2Ugc2lnbmF0dXJl..."
}
```

A client that supports the `com.example.weather-card` extension renders a rich weather card UI with icons, temperature graphs, and forecast. A client that does not support the extension renders:

```
Here's the current weather for Madrid:

Madrid: 22C, Sunny
Humidity: 45%, Wind: 12 km/h
Tomorrow: 24/14C Partly Cloudy
Wednesday: 20/12C Rain
```

---

*AI-Native Messaging Protocol -- Content Specification v0.1.0*
*Cross-references: security.md (trust model, message signing), transport.md (message delivery), threading.md (mentions)*
