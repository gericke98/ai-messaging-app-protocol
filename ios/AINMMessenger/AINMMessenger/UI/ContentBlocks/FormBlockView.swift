import SwiftUI

/// Renders a FormBlock with all 8 field types, client-side validation, and submit button.
/// Per UI-SPEC Component 6: card container, secondarySystemBackground, 12pt corner radius.
/// Action callback wired in Plan 04; defaults to nil for display-only rendering.
struct FormBlockView: View {
    let content: ContentBlock.FormContent
    var onAction: ((_ actionId: String, _ data: [String: Any]) -> Void)? = nil
    @State private var fieldValues: [String: AnyCodableValue] = [:]
    @State private var validationErrors: [String: String] = [:]
    @State private var hasAttemptedSubmit = false
    @State private var isSending = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let title = content.title {
                Text(title)
                    .font(.headline)
                    .fontWeight(.bold)
                    .padding(.bottom, -4)
            }

            if let description = content.description {
                Text(description)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            ForEach(content.fields) { field in
                fieldView(for: field)
            }

            // Submit button
            Button {
                attemptSubmit()
            } label: {
                Text(content.submitLabel ?? "Send Response")
                    .font(.body)
                    .fontWeight(.bold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 44)
                    .foregroundStyle(.white)
                    .background(isSending || hasValidationErrors ? Color(.tertiarySystemFill) : Color.accentColor)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            }
            .disabled(isSending || hasValidationErrors)
        }
        .padding(16)
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .onAppear {
            initializeDefaults()
        }
    }

    private var hasValidationErrors: Bool {
        hasAttemptedSubmit && !validationErrors.isEmpty
    }

    // MARK: - Field Views

    @ViewBuilder
    private func fieldView(for field: ContentBlock.FormField) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            // Label
            HStack(spacing: 0) {
                Text(field.label)
                    .font(.subheadline)
                if field.required == true {
                    Text(" *")
                        .font(.subheadline)
                        .foregroundStyle(.red)
                }
            }

            if let description = field.description {
                Text(description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            // Input
            fieldInput(for: field)

            // Validation error
            if hasAttemptedSubmit, let error = validationErrors[field.id] {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
    }

    @ViewBuilder
    private func fieldInput(for field: ContentBlock.FormField) -> some View {
        switch field.type {
        case "text":
            TextField(
                field.placeholder ?? "",
                text: stringBinding(for: field.id)
            )
            .textFieldStyle(.roundedBorder)

        case "textarea":
            TextEditor(text: stringBinding(for: field.id))
                .frame(minHeight: 80, maxHeight: 200)
                .overlay(
                    RoundedRectangle(cornerRadius: 6)
                        .stroke(Color(.separator), lineWidth: 0.5)
                )

        case "number":
            TextField(
                field.placeholder ?? "",
                text: stringBinding(for: field.id)
            )
            .textFieldStyle(.roundedBorder)
            .keyboardType(.decimalPad)

        case "select":
            Picker(field.label, selection: stringBinding(for: field.id)) {
                Text("Select...").tag("")
                ForEach(field.options ?? []) { option in
                    Text(option.label).tag(option.value)
                }
            }
            .pickerStyle(.menu)

        case "multiselect":
            VStack(alignment: .leading, spacing: 8) {
                ForEach(field.options ?? []) { option in
                    Toggle(option.label, isOn: multiSelectBinding(fieldId: field.id, optionValue: option.value))
                }
            }

        case "checkbox":
            Toggle(
                isOn: boolBinding(for: field.id)
            ) {
                EmptyView()
            }
            .toggleStyle(.switch)

        case "date":
            DatePicker(
                "",
                selection: dateBinding(for: field.id),
                displayedComponents: .date
            )
            .datePickerStyle(.compact)

        case "datetime":
            DatePicker(
                "",
                selection: dateBinding(for: field.id),
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)

        default:
            TextField(
                field.placeholder ?? "",
                text: stringBinding(for: field.id)
            )
            .textFieldStyle(.roundedBorder)
        }
    }

    // MARK: - Bindings

    private func stringBinding(for fieldId: String) -> Binding<String> {
        Binding<String>(
            get: {
                if case .string(let value) = fieldValues[fieldId] {
                    return value
                }
                return ""
            },
            set: { newValue in
                fieldValues[fieldId] = .string(newValue)
                if hasAttemptedSubmit {
                    validateAllFields()
                }
            }
        )
    }

    private func boolBinding(for fieldId: String) -> Binding<Bool> {
        Binding<Bool>(
            get: {
                if case .bool(let value) = fieldValues[fieldId] {
                    return value
                }
                return false
            },
            set: { newValue in
                fieldValues[fieldId] = .bool(newValue)
            }
        )
    }

    private func multiSelectBinding(fieldId: String, optionValue: String) -> Binding<Bool> {
        Binding<Bool>(
            get: {
                if case .array(let values) = fieldValues[fieldId] {
                    return values.contains(.string(optionValue))
                }
                return false
            },
            set: { isSelected in
                var current: [AnyCodableValue] = []
                if case .array(let values) = fieldValues[fieldId] {
                    current = values
                }
                if isSelected {
                    if !current.contains(.string(optionValue)) {
                        current.append(.string(optionValue))
                    }
                } else {
                    current.removeAll { $0 == .string(optionValue) }
                }
                fieldValues[fieldId] = .array(current)
            }
        )
    }

    private func dateBinding(for fieldId: String) -> Binding<Date> {
        Binding<Date>(
            get: {
                if case .string(let value) = fieldValues[fieldId],
                   let date = ISO8601DateFormatter().date(from: value) {
                    return date
                }
                return Date()
            },
            set: { newValue in
                fieldValues[fieldId] = .string(ISO8601DateFormatter().string(from: newValue))
            }
        )
    }

    // MARK: - Initialization & Validation

    private func initializeDefaults() {
        for field in content.fields {
            if let defaultValue = field.defaultValue {
                fieldValues[field.id] = defaultValue
            }
        }
    }

    private func attemptSubmit() {
        hasAttemptedSubmit = true
        validateAllFields()
        guard validationErrors.isEmpty else { return }

        isSending = true
        var formData: [String: Any] = [:]
        for (key, value) in fieldValues {
            formData[key] = anyValue(from: value)
        }
        onAction?("form_submit", formData)
    }

    private func validateAllFields() {
        var errors: [String: String] = [:]
        for field in content.fields {
            if field.required == true {
                let value = fieldValues[field.id]
                let isEmpty: Bool
                switch value {
                case .string(let s): isEmpty = s.trimmingCharacters(in: .whitespaces).isEmpty
                case .array(let a): isEmpty = a.isEmpty
                case .null, .none: isEmpty = true
                default: isEmpty = false
                }
                if isEmpty {
                    errors[field.id] = "Required"
                    continue
                }
            }

            if let pattern = field.validation,
               case .string(let stringValue) = fieldValues[field.id],
               !stringValue.isEmpty {
                if let regex = try? NSRegularExpression(pattern: pattern),
                   regex.firstMatch(
                       in: stringValue,
                       range: NSRange(stringValue.startIndex..., in: stringValue)
                   ) == nil {
                    errors[field.id] = "Invalid format"
                }
            }
        }
        validationErrors = errors
    }

    private func anyValue(from codable: AnyCodableValue) -> Any {
        switch codable {
        case .null: return NSNull()
        case .bool(let v): return v
        case .int(let v): return v
        case .double(let v): return v
        case .string(let v): return v
        case .array(let v): return v.map { anyValue(from: $0) }
        case .object(let v): return v.mapValues { anyValue(from: $0) }
        }
    }
}
