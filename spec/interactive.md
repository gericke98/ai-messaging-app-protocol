# Interactive Content Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Interactive content blocks extend the AINM content block system (defined in content.md) with UI elements that users can interact with. While the Phase 1 core blocks (text, code, image, file) are passive -- rendered for display only -- interactive blocks enable agents to send structured user interface elements that accept user input: buttons, forms, data tables, and approval workflows.

The interactive content system follows a **declarative component model** (see Section 3). The protocol specification defines exactly what fields each interactive block type has. Agents construct blocks using only the fields defined in this specification. Clients render blocks using the fields defined in this specification. There is no arbitrary code execution, no runtime component negotiation, and no dynamic catalog discovery. This specification IS the component catalog. This approach follows the A2UI security model: UI as data, not code.

Interactive blocks are sent by agents (or any participant) as content blocks within standard message envelopes (security.md Section 3.1). They inherit the content block base schema defined in content.md Section 3: every interactive block carries `type`, `version`, `source`, and `fallback` fields. The `fallback` field is mandatory on all interactive blocks, ensuring that clients which do not support interactive content can still display a meaningful text representation.

User interactions with interactive blocks flow back as **ActionBlock** messages. When a user clicks a button, submits a form, or selects an approval option, the client sends a new message in the thread containing an ActionBlock that references the original interactive block. ActionBlock messages are full signed messages (per security.md) delivered to all thread participants via the thread-addressed delivery model (threading.md Section 3.2). This makes user interactions visible to all participants -- a deliberate transparency choice that keeps agents, humans, and system observers informed about every action taken.

This specification defines five block types:

| Block Type | Type String | Purpose | Sent By |
|-----------|-------------|---------|---------|
| **ButtonBlock** | `"button"` | Clickable action trigger | Agent (or any participant) |
| **FormBlock** | `"form"` | Structured data input | Agent (or any participant) |
| **TableBlock** | `"table"` | Tabular data display with optional row actions | Agent (or any participant) |
| **ApprovalBlock** | `"approval"` | Approval workflow with state machine | Agent (or any participant) |
| **ActionBlock** | `"action"` | User response to any interactive block | User (human participant) |

The first four are interactive blocks that present UI to the user. The fifth -- ActionBlock -- is the response mechanism: a content block that captures and communicates the user's interaction. Together, they form a complete request-response pattern for structured human-agent interaction within threads.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Interactive block** | A content block that accepts user input. Interactive blocks extend the content block base schema with type-specific fields for rendering interactive UI elements. The four interactive block types are: ButtonBlock, FormBlock, TableBlock, and ApprovalBlock. |
| **action_id** | A string identifier on an interactive block that uniquely identifies the interaction point within a message. When a user interacts with the block, the resulting ActionBlock references this `action_id` to link the response to the correct interactive element. The `action_id` MUST be unique within a single message's `content[]` array. |
| **ActionBlock** | A content block sent as a new thread message when a user interacts with an interactive block. ActionBlocks carry a reference to the original message and block, the `action_id`, and the user's input data. ActionBlocks use message type `"action"`. |
| **Declarative component model** | The design principle that interactive UI is defined as structured data (JSON fields), not executable code. The protocol specification defines the complete set of interactive block types and their fields. Agents and clients share this specification as a common contract -- no runtime negotiation or discovery is needed. |
| **Semantic style** | A style intent token (`"primary"`, `"secondary"`, `"danger"`) that communicates the purpose of an interactive element. Clients map semantic styles to their own design system. The protocol does not prescribe colors, fonts, or visual treatments -- only intent. |
| **Component catalog** | The set of interactive block types defined by this specification. The catalog is static and defined at the protocol level. There is no runtime catalog discovery mechanism. The specification IS the catalog. |
| **Form field** | A single input element within a FormBlock's `fields` array. Each form field has a `field_type` that determines its rendering and validation behavior. |

## 3. Declarative Component Model

### 3.1 Design Principle

Interactive content blocks use a declarative component model. This means:

1. **The spec defines the components.** This specification defines exactly what fields each interactive block type has. There are no additional block types that can be negotiated at runtime.
2. **Agents construct blocks from the spec.** An agent that wants to show a button constructs a JSON object with `type: "button"` and the fields defined in Section 4 of this specification. The agent does not need to query the client for available components.
3. **Clients render blocks from the spec.** A client that receives a `type: "button"` block renders it according to the fields defined in Section 4. The client does not need to download a component definition or execute agent-provided code.
4. **Unknown fields are ignored.** If a client receives an interactive block with fields not defined in this specification, the client MUST ignore the unknown fields and render the block using only the known fields. This enables forward-compatible evolution: future MINOR version additions (new optional fields) do not break existing clients.
5. **No code execution.** Interactive blocks cannot contain executable code (JavaScript, Swift, WASM, or any other executable format). The declarative model prevents code injection attacks by design.

This approach follows the A2UI security model. Agents reference a known component catalog (this specification) rather than sending arbitrary UI code.

### 3.2 Semantic Styles

The protocol defines three semantic style tokens for interactive elements. These tokens communicate the intent of an action, not its visual appearance. Clients map semantic styles to their own design system (colors, typography, iconography).

| Style | Intent | Typical Usage |
|-------|--------|---------------|
| `"primary"` | Default call-to-action; affirmative action | Submit buttons, approve actions, main interaction |
| `"secondary"` | Alternative or neutral action | Cancel buttons, "Request Changes" options, informational row actions |
| `"danger"` | Destructive or rejecting action | Delete buttons, reject options, actions that cannot be undone |

Conforming clients MUST support all three semantic styles. Clients MUST render `"primary"`, `"secondary"`, and `"danger"` with visually distinct treatments so that users can perceive the intent difference. The protocol does not prescribe specific colors -- a `"danger"` style might be red in one client and orange in another, as long as the user can distinguish it from `"primary"` and `"secondary"`.

If a `style` field contains a value other than these three, clients MUST treat it as `"primary"` (the default).

### 3.3 Core Interactive Block Types

Phase 2 adds four interactive block types and one response block type to the AINM core block set. Combined with the four Phase 1 block types, the complete core block set is:

| Phase | Type String | Block Name | Interactive |
|-------|------------|------------|-------------|
| 1 | `"text"` | TextBlock | No |
| 1 | `"code"` | CodeBlock | No |
| 1 | `"image"` | ImageBlock | No |
| 1 | `"file"` | FileBlock | No |
| 2 | `"button"` | ButtonBlock | Yes |
| 2 | `"form"` | FormBlock | Yes |
| 2 | `"table"` | TableBlock | Yes |
| 2 | `"approval"` | ApprovalBlock | Yes |
| 2 | `"action"` | ActionBlock | Response |

Conforming AINM clients MUST support rendering all core block types from both phases. Clients that do not yet support Phase 2 block types MUST render the `fallback` field (per content.md Section 3).

Extension interactive block types (using reverse-domain namespacing, per content.md Section 5) are allowed but not defined in this specification. Extension interactive blocks follow the same base schema and MAY define their own `action_id` and ActionBlock `data` formats.

## 4. ButtonBlock

ButtonBlock is the simplest interactive block. It renders a clickable button that triggers an ActionBlock when clicked.

**Type:** `"button"`

### 4.1 Schema

```json
{
  "type": "button",
  "version": "1.0",
  "source": "agent",
  "label": "Deploy to Production",
  "action_id": "deploy-prod-001",
  "style": "primary",
  "disabled": false,
  "confirmation": "Are you sure you want to deploy to production? This action cannot be undone.",
  "fallback": "[Button: Deploy to Production]"
}
```

### 4.2 Field Definitions

**Base schema fields** (inherited from content.md Section 3):

- **`type`** (string, REQUIRED): MUST be `"button"`.
- **`version`** (string, REQUIRED): Schema version in `MAJOR.MINOR` format. Current version: `"1.0"`.
- **`source`** (string, REQUIRED): Trust annotation. Typically `"agent"` for agent-sent buttons.
- **`fallback`** (string, REQUIRED): Plain text fallback. Format: `[Button: {label}]`.

**ButtonBlock-specific fields:**

- **`label`** (string, REQUIRED): The text displayed on the button. This is the primary affordance -- it tells the user what action the button performs. Clients MUST render this text prominently.

- **`action_id`** (string, REQUIRED): Unique identifier for this interactive element within the message. When the user clicks the button, the resulting ActionBlock references this `action_id`. The `action_id` MUST be unique within the message's `content[]` array -- no two interactive blocks in the same message may share an `action_id`.

- **`style`** (string, OPTIONAL, default `"primary"`): Semantic style token. One of `"primary"`, `"secondary"`, or `"danger"`. See Section 3.2. If omitted, clients MUST treat the button as `"primary"`.

- **`disabled`** (boolean, OPTIONAL, default `false`): If `true`, the button is rendered in a visually distinct disabled state and is not clickable. Clients MUST NOT send an ActionBlock when a disabled button is clicked. Disabled buttons are useful for showing actions that are not currently available (e.g., a "Merge" button that requires all checks to pass first).

- **`confirmation`** (string or null, OPTIONAL, default `null`): If non-null, this string is a confirmation prompt. When the user clicks the button, the client MUST display this text in a confirmation dialog (or equivalent UI) and require the user to explicitly confirm before sending the ActionBlock. If the user cancels the confirmation, no ActionBlock is sent. This field provides a safety gate for destructive or irreversible actions.

### 4.3 Fallback Format

```
[Button: {label}]
```

Example: `[Button: Deploy to Production]`

Clients that do not support ButtonBlock render this text, making the button's intent clear even in plain text.

### 4.4 Interaction Flow

1. Agent sends a message containing a ButtonBlock with `action_id: "deploy-prod-001"`.
2. Client renders the button with the `label` text, using the `style` for visual treatment.
3. User clicks the button.
4. If `confirmation` is non-null, client shows the confirmation dialog. User confirms.
5. Client sends a new message in the thread with an ActionBlock referencing the ButtonBlock (see Section 8).

### 4.5 Example: Danger Button with Confirmation

```json
{
  "type": "button",
  "version": "1.0",
  "source": "agent",
  "label": "Delete Repository",
  "action_id": "delete-repo-xyz",
  "style": "danger",
  "confirmation": "This will permanently delete the repository 'my-project' and all its data. This action cannot be undone.",
  "fallback": "[Button: Delete Repository]"
}
```

## 5. FormBlock

FormBlock renders a structured form with typed input fields. Forms collect multiple data points from the user in a single interaction.

**Type:** `"form"`

### 5.1 Schema

```json
{
  "type": "form",
  "version": "1.0",
  "source": "agent",
  "action_id": "create-issue-form",
  "title": "Create New Issue",
  "submit_label": "Create Issue",
  "fields": [
    {
      "name": "title",
      "label": "Issue Title",
      "field_type": "text",
      "required": true,
      "placeholder": "Brief description of the issue"
    },
    {
      "name": "description",
      "label": "Description",
      "field_type": "textarea",
      "required": false,
      "placeholder": "Detailed explanation (optional)"
    },
    {
      "name": "priority",
      "label": "Priority",
      "field_type": "select",
      "required": true,
      "options": ["Low", "Medium", "High", "Critical"],
      "default": "Medium"
    },
    {
      "name": "labels",
      "label": "Labels",
      "field_type": "multiselect",
      "required": false,
      "options": ["bug", "feature", "documentation", "performance", "security"]
    },
    {
      "name": "estimated_hours",
      "label": "Estimated Hours",
      "field_type": "number",
      "required": false,
      "placeholder": "0"
    },
    {
      "name": "is_blocking",
      "label": "This is a blocking issue",
      "field_type": "checkbox",
      "required": false,
      "default": false
    },
    {
      "name": "due_date",
      "label": "Due Date",
      "field_type": "date",
      "required": false
    }
  ],
  "fallback": "[Form: Create New Issue - fields: title, description, priority, labels, estimated_hours, is_blocking, due_date]"
}
```

### 5.2 Field Definitions

**Base schema fields** (inherited from content.md Section 3):

- **`type`** (string, REQUIRED): MUST be `"form"`.
- **`version`** (string, REQUIRED): Schema version. Current version: `"1.0"`.
- **`source`** (string, REQUIRED): Trust annotation.
- **`fallback`** (string, REQUIRED): Plain text fallback. Format: `[Form: {title} - fields: {field names comma-separated}]`. If `title` is omitted, use `[Form: fields: {field names comma-separated}]`.

**FormBlock-specific fields:**

- **`action_id`** (string, REQUIRED): Unique identifier for this form within the message. Referenced by the ActionBlock on form submission.

- **`fields`** (array, REQUIRED): Array of field objects defining the form inputs. MUST contain at least one field. Fields are rendered in array order (top to bottom).

- **`title`** (string, OPTIONAL): Form heading displayed above the fields. Informational -- helps the user understand the form's purpose.

- **`submit_label`** (string, OPTIONAL, default `"Submit"`): Text displayed on the form's submit button. Allows agents to provide context-specific button text (e.g., "Create Issue", "Search", "Apply Filters").

### 5.3 Form Field Object

Each element in the `fields` array is a field object with the following properties:

- **`name`** (string, REQUIRED): Field identifier. Used as the key in the ActionBlock's `data` object when the form is submitted. Field names MUST be unique within a single FormBlock's `fields` array.

- **`label`** (string, REQUIRED): Display label rendered next to the field input. Describes the field's purpose to the user.

- **`field_type`** (string, REQUIRED): Determines the input type and rendering. One of the eight field types defined below.

- **`required`** (boolean, OPTIONAL, default `false`): If `true`, the field must have a value before the form can be submitted. Clients MUST validate required fields before submission and MUST display a validation error if a required field is empty.

- **`default`** (any, OPTIONAL): Default value for the field. The type of this value MUST match the field's `field_type` (see Section 5.4 for type-specific default value types). If provided, the field is pre-populated with this value when the form is rendered.

- **`placeholder`** (string, OPTIONAL): Hint text displayed inside empty `text`, `textarea`, and `number` fields. Placeholder text disappears when the user begins typing. Clients SHOULD render placeholder text in a visually distinct style (e.g., lighter color) to distinguish it from user input.

- **`validation`** (string, OPTIONAL): A regular expression pattern for client-side validation. Clients SHOULD validate the field value against this pattern before form submission. If validation fails, clients SHOULD display a validation error and prevent submission. The regex syntax follows ECMAScript regular expressions (ECMA-262). The pattern is tested against the complete field value (implicit `^` and `$` anchors).

- **`options`** (array of strings, REQUIRED for `select` and `multiselect`): The list of available choices. Each string in the array is both the display label and the submitted value. Clients MUST render all options. For `select`, exactly one option may be chosen. For `multiselect`, zero or more options may be chosen.

### 5.4 Form Field Types

| Field Type | Input | Value Type | Default Type | Notes |
|-----------|-------|-----------|-------------|-------|
| `text` | Single-line text input | string | string | Supports `placeholder` and `validation` |
| `textarea` | Multi-line text input | string | string | Supports `placeholder` and `validation` |
| `number` | Numeric input | number | number | Supports `placeholder` and `validation` |
| `select` | Dropdown / picker | string | string | REQUIRES `options` array. Single selection. |
| `multiselect` | Multi-choice selector | array of strings | array of strings | REQUIRES `options` array. Multiple selections. |
| `checkbox` | Boolean toggle | boolean | boolean | Renders as a checkbox or toggle. Label text from `label` field. |
| `date` | Date picker | string (ISO 8601 date: `YYYY-MM-DD`) | string | Calendar date without time component. |
| `datetime` | Date and time picker | string (RFC 3339: `YYYY-MM-DDTHH:MM:SSZ`) | string | Full date and time with timezone. |

Clients MUST support all eight field types. If a client encounters an unrecognized `field_type`, it SHOULD render a text input as a fallback.

### 5.5 Form Validation

Client-side validation occurs before form submission:

1. **Required fields:** Clients MUST check that all fields with `required: true` have a non-empty value. An empty string, `null`, or an empty array (for multiselect) counts as empty.
2. **Regex validation:** Clients SHOULD check fields with a `validation` pattern against the field value. The pattern is tested against the complete value.
3. **Type validation:** Clients SHOULD verify that field values match the expected type for the `field_type` (e.g., a `number` field should contain a valid number).

If validation fails, the client MUST NOT submit the form. The client SHOULD display validation errors inline next to the failing fields.

Server-side validation is OPTIONAL. Agents receiving the ActionBlock MAY perform additional validation on the submitted data and respond with appropriate feedback.

### 5.6 Fallback Format

```
[Form: {title} - fields: {field1_name}, {field2_name}, ...]
```

Example: `[Form: Create New Issue - fields: title, description, priority, labels, estimated_hours, is_blocking, due_date]`

If `title` is omitted:

```
[Form: fields: {field1_name}, {field2_name}, ...]
```

## 6. TableBlock

TableBlock renders structured tabular data with optional per-row action buttons. Tables display information in a column-and-row format with support for client-side sorting.

**Type:** `"table"`

### 6.1 Schema

```json
{
  "type": "table",
  "version": "1.0",
  "source": "agent",
  "title": "Open Pull Requests",
  "columns": [
    { "key": "number", "label": "#", "sortable": true, "align": "right" },
    { "key": "title", "label": "Title", "sortable": true, "align": "left" },
    { "key": "author", "label": "Author", "sortable": true, "align": "left" },
    { "key": "status", "label": "Status", "sortable": false, "align": "center" },
    { "key": "updated", "label": "Updated", "sortable": true, "align": "left" }
  ],
  "rows": [
    {
      "number": 142,
      "title": "Add streaming support",
      "author": "alice",
      "status": "Ready for review",
      "updated": "2026-03-28"
    },
    {
      "number": 139,
      "title": "Fix memory leak in WebSocket handler",
      "author": "bob",
      "status": "Changes requested",
      "updated": "2026-03-27"
    },
    {
      "number": 137,
      "title": "Update dependencies",
      "author": "dependabot",
      "status": "Approved",
      "updated": "2026-03-26"
    }
  ],
  "row_actions": [
    { "action_id": "review-pr", "label": "Review", "style": "primary" },
    { "action_id": "merge-pr", "label": "Merge", "style": "secondary" }
  ],
  "fallback": "Open Pull Requests\n\n| # | Title | Author | Status | Updated |\n|---|-------|--------|--------|---------|\n| 142 | Add streaming support | alice | Ready for review | 2026-03-28 |\n| 139 | Fix memory leak in WebSocket handler | bob | Changes requested | 2026-03-27 |\n| 137 | Update dependencies | dependabot | Approved | 2026-03-26 |"
}
```

### 6.2 Field Definitions

**Base schema fields** (inherited from content.md Section 3):

- **`type`** (string, REQUIRED): MUST be `"table"`.
- **`version`** (string, REQUIRED): Schema version. Current version: `"1.0"`.
- **`source`** (string, REQUIRED): Trust annotation.
- **`fallback`** (string, REQUIRED): A Markdown table representation of the data, including headers and all rows. See Section 6.6.

**TableBlock-specific fields:**

- **`columns`** (array, REQUIRED): Array of column definition objects. Defines the table structure. Columns are rendered in array order (left to right). MUST contain at least one column.

- **`rows`** (array, REQUIRED): Array of row objects. Each row object has keys that correspond to column `key` values. MAY be empty (zero rows).

- **`title`** (string, OPTIONAL): Table heading displayed above the table.

- **`row_actions`** (array, OPTIONAL): Array of action button definitions rendered for each row. When a user clicks a row action, the resulting ActionBlock includes the row's data. See Section 6.5.

### 6.3 Column Definition Object

Each element in the `columns` array defines a table column:

- **`key`** (string, REQUIRED): The key used to extract this column's value from each row object. MUST correspond to a property name in the row objects.

- **`label`** (string, REQUIRED): Column header text displayed to the user.

- **`sortable`** (boolean, OPTIONAL, default `false`): If `true`, the client MUST allow the user to sort the table by this column. Sorting is client-side only -- no server interaction. Clients SHOULD support ascending and descending sort toggling.

- **`align`** (string, OPTIONAL, default `"left"`): Text alignment for this column. One of `"left"`, `"right"`, or `"center"`. Applies to both the header and all data cells.

### 6.4 Row Objects

Row objects are plain JSON objects whose keys correspond to column `key` values. Row values may be strings, numbers, booleans, or null. Clients render values as text.

If a row object is missing a key that corresponds to a defined column, the client MUST render an empty cell.

### 6.5 Row Actions

Row actions are action buttons rendered alongside each row (typically in an actions column at the end of the table). Each row action object has:

- **`action_id`** (string, REQUIRED): The identifier referenced by the ActionBlock when the user clicks this action for a specific row.

- **`label`** (string, REQUIRED): Button text for the row action.

- **`style`** (string, OPTIONAL, default `"primary"`): Semantic style. One of `"primary"`, `"secondary"`, or `"danger"`.

When a user clicks a row action, the client sends an ActionBlock with `data` containing the full row object (all key-value pairs for that row). This provides the agent with the complete row context.

### 6.6 Fallback Format

The fallback for TableBlock is a Markdown table including headers and all rows:

```
{title}

| {col1_label} | {col2_label} | ... |
|---|---|---|
| {row1_col1} | {row1_col2} | ... |
| {row2_col1} | {row2_col2} | ... |
```

If `title` is omitted, the fallback starts with the header row.

### 6.7 Sorting

Sorting is client-side only. The server does not participate in sort operations.

- If a column has `sortable: true`, the client MUST provide a sort affordance (e.g., clickable column header).
- Clicking a sortable column header SHOULD toggle between ascending sort, descending sort, and unsorted (original order).
- Sorting applies to the displayed rows only. It does not modify the underlying data or trigger any server communication.

### 6.8 Pagination

Pagination is deferred to future protocol versions. In v1, all rows are included inline in the `rows` array. For large datasets, agents SHOULD limit the number of rows to a reasonable amount (recommended maximum: 100 rows) and indicate in an accompanying TextBlock if the data is truncated.

## 7. ApprovalBlock

ApprovalBlock renders an approval workflow with a state machine. Approvals are used when an agent requests explicit human authorization before proceeding with an action.

**Type:** `"approval"`

### 7.1 Schema

```json
{
  "type": "approval",
  "version": "1.0",
  "source": "agent",
  "action_id": "approve-deployment-v2",
  "title": "Production Deployment Approval",
  "description": "Deploy version 2.4.1 to production (us-east-1). This will affect 12,000 active users. Rollback plan: automatic rollback on error rate > 5%.",
  "status": "pending",
  "options": [
    { "value": "approved", "label": "Approve", "style": "primary" },
    { "value": "rejected", "label": "Reject", "style": "danger" },
    { "value": "changes_requested", "label": "Request Changes", "style": "secondary" }
  ],
  "fallback": "[Approval: Production Deployment Approval - Status: pending]"
}
```

### 7.2 Field Definitions

**Base schema fields** (inherited from content.md Section 3):

- **`type`** (string, REQUIRED): MUST be `"approval"`.
- **`version`** (string, REQUIRED): Schema version. Current version: `"1.0"`.
- **`source`** (string, REQUIRED): Trust annotation.
- **`fallback`** (string, REQUIRED): Format: `[Approval: {title} - Status: {status}]`.

**ApprovalBlock-specific fields:**

- **`action_id`** (string, REQUIRED): Unique identifier for this approval within the message. Referenced by the ActionBlock when a user selects an option.

- **`title`** (string, REQUIRED): A concise description of what is being approved. This is the primary text displayed to the user.

- **`description`** (string, OPTIONAL): Additional details about the approval. Agents SHOULD provide enough context for the approver to make an informed decision without needing to ask follow-up questions.

- **`status`** (string, REQUIRED): Current approval state. One of `"pending"`, `"approved"`, `"rejected"`, or `"changes_requested"`. The initial value when the agent sends the ApprovalBlock MUST be `"pending"`. See Section 7.3 for state transitions.

- **`options`** (array, REQUIRED): Array of option objects representing the available approval decisions. MUST contain at least one option. Each option object has:

  - **`value`** (string, REQUIRED): The option value submitted in the ActionBlock's `data` field. Should match one of the valid terminal states (`"approved"`, `"rejected"`, `"changes_requested"`).
  - **`label`** (string, REQUIRED): Display text for the option button.
  - **`style`** (string, OPTIONAL, default `"primary"`): Semantic style for the option button.

### 7.3 State Machine

ApprovalBlock follows a simple state machine with one initial state and three terminal states:

```
                    +----------+
                    | pending  |  (initial state)
                    +----+-----+
                         |
            +------------+------------+
            |            |            |
            v            v            v
       +--------+  +----------+  +------------------+
       |approved|  | rejected |  |changes_requested |
       +--------+  +----------+  +------------------+
       (terminal)  (terminal)    (terminal)
```

**State definitions:**

| State | Description | Terminal |
|-------|-------------|----------|
| `pending` | Awaiting a decision. Options are clickable. | No |
| `approved` | The request was approved. | Yes |
| `rejected` | The request was rejected. | Yes |
| `changes_requested` | The approver requested modifications before re-submission. | Yes |

**Transition rules:**

- The only valid transitions are from `pending` to any of the three terminal states.
- All terminal states are **immutable**. Once an ApprovalBlock reaches a terminal state, no further state transitions are allowed.
- Clients MUST NOT render clickable options on ApprovalBlocks in a terminal state. Clients SHOULD visually indicate which option was selected and display the resolved status.
- The server MUST reject ActionBlocks targeting ApprovalBlocks that are no longer in the `pending` state (see Section 10, error code `-32071`).

### 7.4 Approval Lifecycle

1. Agent sends a message containing an ApprovalBlock with `status: "pending"`.
2. Client renders the approval with all options as clickable buttons.
3. User selects an option (e.g., "Approve").
4. Client sends an ActionBlock message in the thread with `data: { "value": "approved" }`.
5. Server processes the ActionBlock, transitions the ApprovalBlock status to `"approved"`.
6. All thread participants see the ActionBlock and can observe the approval outcome.
7. Client re-renders the ApprovalBlock showing the resolved state with the selected option highlighted and other options greyed out.

### 7.5 Server Enforcement

The server is the authority on ApprovalBlock state. When the server receives an ActionBlock targeting an ApprovalBlock:

- If the ApprovalBlock is in `pending` state: process the action, transition to the selected terminal state.
- If the ApprovalBlock is in any terminal state: reject the ActionBlock with error `-32071` (Approval already resolved).

This server-side enforcement prevents race conditions where two participants attempt to resolve the same approval simultaneously. The first ActionBlock received by the server wins; the second is rejected.

### 7.6 Fallback Format

```
[Approval: {title} - Status: {status}]
```

Example: `[Approval: Production Deployment Approval - Status: pending]`

After resolution: `[Approval: Production Deployment Approval - Status: approved]`

## 8. ActionBlock

ActionBlock is the user response mechanism for interactive blocks. When a user interacts with any interactive block (clicks a button, submits a form, clicks a table row action, or selects an approval option), the client creates and sends a new message in the thread containing an ActionBlock.

ActionBlock is NOT an interactive block itself -- it is a response block. ActionBlocks are not clickable or interactive. They are a permanent record of the user's interaction, visible to all thread participants.

**Type:** `"action"`

### 8.1 Schema

```json
{
  "type": "action",
  "version": "1.0",
  "source": "user",
  "ref_message_id": "019580b1-cccc-7000-8000-000000000003",
  "ref_block_index": 2,
  "action_id": "deploy-prod-001",
  "data": {},
  "fallback": "[Action: clicked 'Deploy to Production']"
}
```

### 8.2 Field Definitions

**Base schema fields** (inherited from content.md Section 3):

- **`type`** (string, REQUIRED): MUST be `"action"`.
- **`version`** (string, REQUIRED): Schema version. Current version: `"1.0"`.
- **`source`** (string, REQUIRED): Trust annotation. ActionBlocks represent human input, so this SHOULD be `"user"`. An agent that programmatically responds to an interactive block would set `source: "agent"`.
- **`fallback`** (string, REQUIRED): Plain text fallback describing the action. See Section 8.5.

**ActionBlock-specific fields:**

- **`ref_message_id`** (string, REQUIRED): The `id` (UUIDv7) of the message containing the interactive block that was interacted with. This links the ActionBlock to the original message. The referenced message MUST exist in the same thread.

- **`ref_block_index`** (integer, REQUIRED): The zero-based index of the interactive block within the referenced message's `content[]` array. Combined with `ref_message_id`, this uniquely identifies the specific interactive block. The index MUST be a valid index into the referenced message's `content[]` array, and the block at that index MUST be an interactive block type.

- **`action_id`** (string, REQUIRED): MUST match the `action_id` of the interactive block at position `ref_block_index` in the referenced message. This provides an additional verification layer -- even if the `ref_block_index` is correct, the `action_id` must also match.

- **`data`** (object, REQUIRED): User input data. The structure depends on the interactive block type:

  | Source Block | `data` Format | Example |
  |-------------|---------------|---------|
  | ButtonBlock | Empty object `{}` (buttons have no input data) | `{}` |
  | FormBlock | Object with field `name` keys and user input values | `{"title": "Bug report", "priority": "High"}` |
  | TableBlock (row action) | Object containing the full row values | `{"number": 142, "title": "Add streaming support", "author": "alice"}` |
  | ApprovalBlock | Object with `value` key matching the selected option | `{"value": "approved"}` |

### 8.3 Message Type

ActionBlock messages use message type `"action"` in the message envelope (security.md Section 3.3). This is a new message type added in Phase 2, extending the Phase 1 set:

| Type Value | Phase | Description |
|-----------|-------|-------------|
| `message` | 1 | Standard message with content blocks |
| `thread.create` | 1 | Creates a new thread |
| `thread.join` | 1 | Participant joins a thread |
| `thread.leave` | 1 | Participant leaves a thread |
| `presence.update` | 1 | Updates presence status |
| `action` | 2 | User response to an interactive block |

ActionBlock messages are full signed messages. They follow the complete signing procedure defined in security.md Section 4. The message envelope contains the standard fields (`id`, `from`, `thread_id`, `timestamp`, `type`, `content`, `signature`), where `type` is `"action"` and `content` contains the ActionBlock.

### 8.4 Deduplication

ActionBlock messages are subject to the same deduplication rules as all other messages (security.md Section 7). The server MUST deduplicate ActionBlock messages by their `id` field (UUIDv7). If a server receives two ActionBlock messages with the same `id`, it MUST process only the first and silently discard the second.

This is the primary defense against ActionBlock replay attacks. Each user interaction generates a unique UUIDv7 message `id`. Replaying a previously valid ActionBlock will be caught by the deduplication mechanism.

### 8.5 Fallback Format

The fallback format depends on the source interactive block:

| Source Block | Fallback Format | Example |
|-------------|----------------|---------|
| ButtonBlock | `[Action: clicked '{label}']` | `[Action: clicked 'Deploy to Production']` |
| FormBlock | `[Action: submitted '{title}']` | `[Action: submitted 'Create New Issue']` |
| TableBlock | `[Action: '{row_action_label}' on row {key}={value}]` | `[Action: 'Review' on row #=142]` |
| ApprovalBlock | `[Action: {selected_label} on '{title}']` | `[Action: Approve on 'Production Deployment Approval']` |

If the source block's title or label is not available, clients SHOULD use the `action_id` as a fallback identifier.

### 8.6 Example: Complete ActionBlock Message Envelope

A user clicking the "Deploy to Production" button from Section 4 triggers this message:

```json
{
  "id": "019580d2-1111-7000-8000-aaaaaaaaa001",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:45:30.123Z",
  "type": "action",
  "content": [
    {
      "type": "action",
      "version": "1.0",
      "source": "user",
      "ref_message_id": "019580b1-cccc-7000-8000-000000000003",
      "ref_block_index": 0,
      "action_id": "deploy-prod-001",
      "data": {},
      "fallback": "[Action: clicked 'Deploy to Production']"
    }
  ],
  "signature": "YWxpY2UtY2xpY2tlZC1kZXBsb3ktYnV0dG9uLXNpZ25hdHVyZQ"
}
```

### 8.7 Example: Form Submission ActionBlock

A user submitting the "Create New Issue" form from Section 5 triggers this message:

```json
{
  "id": "019580d3-2222-7000-8000-bbbbbbbbb002",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:46:15.456Z",
  "type": "action",
  "content": [
    {
      "type": "action",
      "version": "1.0",
      "source": "user",
      "ref_message_id": "019580b1-dddd-7000-8000-000000000004",
      "ref_block_index": 0,
      "action_id": "create-issue-form",
      "data": {
        "title": "WebSocket connection drops after 30 seconds",
        "description": "When connected via cellular network, the WebSocket connection is dropped after exactly 30 seconds. This appears to be a keepalive issue.",
        "priority": "High",
        "labels": ["bug", "performance"],
        "estimated_hours": 4,
        "is_blocking": true,
        "due_date": "2026-04-05"
      },
      "fallback": "[Action: submitted 'Create New Issue']"
    }
  ],
  "signature": "YWxpY2Utc3VibWl0dGVkLWlzc3VlLWZvcm0tc2lnbmF0dXJl"
}
```

## 9. Content Size Limits

The AINM protocol does NOT impose arbitrary payload size limits on interactive content blocks or messages in general. This is a deliberate design choice.

### 9.1 Protocol-Level Position

The protocol specification defines the message format and block schemas, not operational constraints. Size limits are operational concerns that vary by deployment:

- A personal messaging server may accept messages up to 256KB.
- An enterprise deployment may accept messages up to 10MB.
- A public relay server may impose stricter limits to prevent abuse.

Imposing fixed limits in the protocol specification would either be too restrictive for legitimate use cases or too permissive for constrained deployments.

### 9.2 Server-Configurable Limits

Servers communicate their operational limits during capability negotiation (transport.md Section 4.3). Two capability flags are relevant:

- **`maxContentBlocks`** (integer): Maximum number of content blocks per message. Includes all block types (static and interactive).
- **`maxMessageSize`** (integer, bytes): Maximum total size of a message envelope in bytes.

These are server-configurable values, not protocol-imposed constants. Servers SHOULD accept messages up to 1MB by default. Servers MAY impose stricter or more permissive limits based on their operational requirements.

### 9.3 Client Behavior

Clients SHOULD respect the server's advertised limits. Before sending a message that may exceed the `maxMessageSize` limit, clients SHOULD check the message size and warn the user. If a server rejects a message for exceeding size limits, the server MUST return a descriptive error (the specific error code is defined by the transport specification).

### 9.4 Interactive Block Considerations

Interactive blocks -- particularly FormBlocks with many fields and TableBlocks with many rows -- can produce large payloads. Agents constructing these blocks SHOULD be mindful of practical payload sizes:

- **FormBlock:** Agents SHOULD limit forms to a reasonable number of fields (recommended maximum: 20 fields). Deeply nested or overly complex forms degrade the user experience regardless of protocol limits.
- **TableBlock:** Agents SHOULD limit inline rows to a reasonable count (recommended maximum: 100 rows). For larger datasets, agents SHOULD paginate by sending multiple messages or providing a link to a full dataset.

These are recommendations, not protocol constraints. Servers and clients enforce their own limits through capability negotiation.

## 10. Error Handling

Interactive content errors use JSON-RPC 2.0 error codes in the range `-32070` to `-32079`.

### 10.1 Error Code Table

| Code | Name | Description | When Returned |
|------|------|-------------|---------------|
| `-32070` | Invalid Action ID | The `action_id` in an ActionBlock does not match any interactive block in the referenced message. | Server rejects ActionBlock: the `action_id` field does not correspond to an interactive block at the specified `ref_block_index`. |
| `-32071` | Approval Already Resolved | An ActionBlock targets an ApprovalBlock that is no longer in `pending` state. | Server rejects ActionBlock: the referenced ApprovalBlock has already been resolved (approved, rejected, or changes_requested). |
| `-32072` | Form Validation Failed | An ActionBlock for a FormBlock fails server-side validation. | Server rejects ActionBlock: a required field is missing or a validation regex check failed on submitted form data. |
| `-32073` | Referenced Message Not Found | The `ref_message_id` in an ActionBlock does not correspond to any message in the thread. | Server rejects ActionBlock: the referenced message does not exist or is not in the same thread. |
| `-32074` | Invalid Block Index | The `ref_block_index` in an ActionBlock is out of range for the referenced message's `content[]` array, or the block at that index is not an interactive block type. | Server rejects ActionBlock: the index is negative, exceeds the array length, or points to a non-interactive block. |

### 10.2 Error Response Format

Interactive content errors follow the standard JSON-RPC 2.0 error response format (see security.md Section 8.2):

```json
{
  "jsonrpc": "2.0",
  "id": 55,
  "error": {
    "code": -32071,
    "message": "Approval already resolved",
    "data": {
      "ref_message_id": "019580b1-cccc-7000-8000-000000000003",
      "action_id": "approve-deployment-v2",
      "current_status": "approved",
      "resolved_by": "bob@messaging.example.com",
      "resolved_at": "2026-03-28T12:44:00.000Z"
    }
  }
}
```

The `data` field is OPTIONAL. When provided, it SHOULD include diagnostic information relevant to the specific error:

- For `-32070`: the `action_id` that was not found, the `ref_message_id`, and the `ref_block_index`.
- For `-32071`: the current `status`, who resolved it (`resolved_by`), and when (`resolved_at`).
- For `-32072`: which field failed validation and why.
- For `-32073`: the `ref_message_id` that was not found.
- For `-32074`: the `ref_block_index` and the actual length of the `content[]` array.

### 10.3 Client Error Handling

When a client receives an error response after sending an ActionBlock:

- **`-32070` (Invalid Action ID):** This indicates a client bug or a stale reference. The client SHOULD log the error and display a generic "Action failed" message. This should not happen if the client correctly tracks `action_id` values.
- **`-32071` (Approval Already Resolved):** The client SHOULD update the ApprovalBlock rendering to show the resolved state and inform the user that someone else already resolved the approval.
- **`-32072` (Form Validation Failed):** The client SHOULD display the server's validation error to the user and allow them to correct and resubmit the form.
- **`-32073` (Referenced Message Not Found):** The client SHOULD display a "Message no longer available" error. This may occur if the referenced message was deleted or the client has stale data.
- **`-32074` (Invalid Block Index):** Same handling as `-32070` -- indicates a client bug.

## 11. Security Considerations

### 11.1 ActionBlocks as Signed Messages

ActionBlocks are full signed messages, not ephemeral events. Every ActionBlock message follows the complete signing procedure defined in security.md Section 4:

1. The ActionBlock message envelope is constructed with all fields (`id`, `from`, `thread_id`, `timestamp`, `type`, `content`, but without `signature`).
2. The envelope is canonicalized using RFC 8785 (JSON Canonicalization Scheme).
3. The canonical form is signed with the sender's Ed25519 private key.
4. The signature is added to the envelope.

This means every user interaction with an interactive block produces a cryptographically signed, tamper-evident record. Recipients can verify that the action was taken by the claimed sender and that the action data has not been modified.

### 11.2 Transparency by Design

ActionBlocks are delivered to all thread participants via thread-addressed delivery (threading.md Section 3.2). This is intentional transparency:

- When Alice clicks "Approve" on a deployment approval, all thread participants (including other humans and agents) see Alice's ActionBlock in the thread.
- When Bob submits a form, all participants see the submitted data.
- When an agent clicks a table row action, all participants see which row was acted upon.

This design prevents hidden side-channels. There is no mechanism for "private" interactions with interactive blocks -- all actions are visible to all thread participants.

### 11.3 Confirmation Gate for Destructive Actions

The `confirmation` field on ButtonBlock provides a protocol-level security gate for destructive or irreversible actions. When `confirmation` is non-null:

1. The client MUST display the confirmation text to the user.
2. The client MUST require explicit user confirmation (e.g., a "Confirm" button in a dialog).
3. Only after the user confirms does the client send the ActionBlock.

This prevents accidental clicks from triggering destructive actions. Agents SHOULD use the `confirmation` field on any button that performs an action that cannot be easily undone (deployments, deletions, financial transactions).

### 11.4 No Code Execution

Interactive blocks follow the declarative component model (Section 3). This means:

- Agents CANNOT include executable code (JavaScript, WASM, etc.) in interactive blocks.
- Clients MUST NOT execute any code found in interactive block fields.
- The `data` field in ActionBlocks contains only JSON values, never executable code.

This prevents code injection attacks through the interactive content system. A malicious agent cannot craft an interactive block that executes arbitrary code on the client. The worst a malicious agent can do is construct valid interactive blocks with misleading labels or descriptions -- which is mitigated by the trust annotation system (the `source` field identifies who constructed the block).

### 11.5 Trust Annotations on ActionBlocks

ActionBlocks representing human input carry `source: "user"`. This allows agents and other participants to distinguish human actions from automated actions:

- `source: "user"` -- A human clicked the button, submitted the form, or selected the approval option.
- `source: "agent"` -- An agent programmatically responded to an interactive block (e.g., an automated approval agent).

As with all trust annotations (security.md Section 6), the `source` field is covered by the message signature and is tamper-evident. Cross-checking against the sender's identity type (human vs. agent) provides additional verification (security.md Section 6.3).

## 12. Examples

### 12.1 End-to-End Workflow: Form Submission + Approval

This example demonstrates the complete interactive content lifecycle: an agent sends a form and an approval request, a user submits the form, and another user approves the request.

**Step 1: Agent sends a message with FormBlock + ApprovalBlock**

The deployment agent sends a message requesting deployment details and approval:

```json
{
  "id": "019580e0-aaaa-7000-8000-000000000001",
  "from": "deploy-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T13:00:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Ready to deploy. Please configure the deployment and approve when ready.",
      "fallback": "Ready to deploy. Please configure the deployment and approve when ready."
    },
    {
      "type": "form",
      "version": "1.0",
      "source": "agent",
      "action_id": "deploy-config",
      "title": "Deployment Configuration",
      "submit_label": "Save Configuration",
      "fields": [
        {
          "name": "environment",
          "label": "Target Environment",
          "field_type": "select",
          "required": true,
          "options": ["staging", "production"],
          "default": "staging"
        },
        {
          "name": "version",
          "label": "Version Tag",
          "field_type": "text",
          "required": true,
          "placeholder": "e.g., v2.4.1",
          "validation": "^v\\d+\\.\\d+\\.\\d+$"
        },
        {
          "name": "notify_team",
          "label": "Notify team on completion",
          "field_type": "checkbox",
          "default": true
        }
      ],
      "fallback": "[Form: Deployment Configuration - fields: environment, version, notify_team]"
    },
    {
      "type": "approval",
      "version": "1.0",
      "source": "agent",
      "action_id": "approve-deploy",
      "title": "Approve Deployment",
      "description": "Approval required before deployment proceeds. The configured environment and version will be deployed.",
      "status": "pending",
      "options": [
        { "value": "approved", "label": "Approve & Deploy", "style": "primary" },
        { "value": "rejected", "label": "Cancel Deployment", "style": "danger" },
        { "value": "changes_requested", "label": "Request Changes", "style": "secondary" }
      ],
      "fallback": "[Approval: Approve Deployment - Status: pending]"
    }
  ],
  "signature": "ZGVwbG95LWJvdC1zZW5kcy1mb3JtLWFuZC1hcHByb3ZhbA"
}
```

**Step 2: Alice submits the form**

Alice fills out the deployment configuration and submits:

```json
{
  "id": "019580e1-bbbb-7000-8000-000000000002",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T13:02:30.000Z",
  "type": "action",
  "content": [
    {
      "type": "action",
      "version": "1.0",
      "source": "user",
      "ref_message_id": "019580e0-aaaa-7000-8000-000000000001",
      "ref_block_index": 1,
      "action_id": "deploy-config",
      "data": {
        "environment": "production",
        "version": "v2.4.1",
        "notify_team": true
      },
      "fallback": "[Action: submitted 'Deployment Configuration']"
    }
  ],
  "signature": "YWxpY2Utc3VibWl0cy1kZXBsb3ktY29uZmln"
}
```

Note: `ref_block_index` is `1` because the FormBlock is the second block (index 1) in the original message's `content[]` array.

**Step 3: Bob approves the deployment**

Bob reviews the configuration Alice submitted and approves:

```json
{
  "id": "019580e2-cccc-7000-8000-000000000003",
  "from": "bob@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T13:05:00.000Z",
  "type": "action",
  "content": [
    {
      "type": "action",
      "version": "1.0",
      "source": "user",
      "ref_message_id": "019580e0-aaaa-7000-8000-000000000001",
      "ref_block_index": 2,
      "action_id": "approve-deploy",
      "data": {
        "value": "approved"
      },
      "fallback": "[Action: Approve & Deploy on 'Approve Deployment']"
    }
  ],
  "signature": "Ym9iLWFwcHJvdmVzLWRlcGxveW1lbnQ"
}
```

Note: `ref_block_index` is `2` because the ApprovalBlock is the third block (index 2) in the original message's `content[]` array.

**Thread timeline as seen by all participants:**

1. **deploy-bot:** "Ready to deploy. Please configure the deployment and approve when ready." + [Form: Deployment Configuration] + [Approval: Approve Deployment - Status: pending]
2. **alice:** [Action: submitted 'Deployment Configuration'] -- environment=production, version=v2.4.1, notify_team=true
3. **bob:** [Action: Approve & Deploy on 'Approve Deployment']

All three messages are signed, timestamped, and visible to every participant in the thread. The agent, Alice, Bob, and any other participants can see the complete interaction history.

### 12.2 Table with Row Actions

An agent displays a list of servers with management actions:

```json
{
  "id": "019580f0-dddd-7000-8000-000000000004",
  "from": "infra-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T13:10:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Here are the servers in your cluster:",
      "fallback": "Here are the servers in your cluster:"
    },
    {
      "type": "table",
      "version": "1.0",
      "source": "agent",
      "title": "Cluster Servers",
      "columns": [
        { "key": "name", "label": "Server", "sortable": true },
        { "key": "region", "label": "Region", "sortable": true },
        { "key": "cpu", "label": "CPU %", "sortable": true, "align": "right" },
        { "key": "memory", "label": "Memory %", "sortable": true, "align": "right" },
        { "key": "status", "label": "Status", "sortable": false, "align": "center" }
      ],
      "rows": [
        { "name": "web-01", "region": "us-east-1", "cpu": 45, "memory": 62, "status": "healthy" },
        { "name": "web-02", "region": "us-east-1", "cpu": 78, "memory": 85, "status": "warning" },
        { "name": "api-01", "region": "eu-west-1", "cpu": 23, "memory": 41, "status": "healthy" }
      ],
      "row_actions": [
        { "action_id": "restart-server", "label": "Restart", "style": "danger" },
        { "action_id": "view-logs", "label": "View Logs", "style": "secondary" }
      ],
      "fallback": "Cluster Servers\n\n| Server | Region | CPU % | Memory % | Status |\n|--------|--------|-------|----------|--------|\n| web-01 | us-east-1 | 45 | 62 | healthy |\n| web-02 | us-east-1 | 78 | 85 | warning |\n| api-01 | eu-west-1 | 23 | 41 | healthy |"
    }
  ],
  "signature": "aW5mcmEtYm90LXNlcnZlci1saXN0LXNpZ25hdHVyZQ"
}
```

A user clicking "Restart" on the `web-02` row triggers this ActionBlock:

```json
{
  "id": "019580f1-eeee-7000-8000-000000000005",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T13:11:30.000Z",
  "type": "action",
  "content": [
    {
      "type": "action",
      "version": "1.0",
      "source": "user",
      "ref_message_id": "019580f0-dddd-7000-8000-000000000004",
      "ref_block_index": 1,
      "action_id": "restart-server",
      "data": {
        "name": "web-02",
        "region": "us-east-1",
        "cpu": 78,
        "memory": 85,
        "status": "warning"
      },
      "fallback": "[Action: 'Restart' on row Server=web-02]"
    }
  ],
  "signature": "YWxpY2UtcmVzdGFydHMtd2ViLTAyLXNlcnZlcg"
}
```

The `data` object contains the complete row values, giving the agent full context about which server was targeted.

---

*AI-Native Messaging Protocol -- Interactive Content Specification v0.1.0*
*Cross-references: content.md (base schema, core block types, extension model), security.md (message signing, trust annotations, deduplication), threading.md (thread-addressed delivery, message visibility)*
