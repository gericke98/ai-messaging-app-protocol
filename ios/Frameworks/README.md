# iOS Frameworks

## AinmEscrow.xcframework

A **static-library** xcframework wrapping the `ainm-escrow` crypto crate (the same
crate the server uses, so escrow recovery is wire-compatible by construction) via
UniFFI 0.31.

This binary is **not checked into git** (see `.gitignore`). Regenerate it with:

```bash
bash scripts/build-escrow-xcframework.sh
```

The script:
1. Builds `libainm_escrow_ffi.a` for `aarch64-apple-ios`, `aarch64-apple-ios-sim`,
   and `x86_64-apple-ios`.
2. Generates the Swift bindings into
   `ios/AINMMessenger/AINMMessenger/Generated/ainm_escrow_ffi.swift` (this `.swift`
   IS committed — it is the high-level API the app imports).
3. `lipo`s the two simulator arches into one universal slice.
4. Assembles `AinmEscrow.xcframework` (device slice + simulator slice), each with the
   generated C header + `module.modulemap`.

### Xcode wiring (already in `project.pbxproj`)

- The xcframework is linked in the app target's **Frameworks** build phase
  (it is a static lib, so it is linked — not embedded/code-signed-on-copy).
- `FRAMEWORK_SEARCH_PATHS` includes `$(SRCROOT)/../Frameworks`.
- `Generated/ainm_escrow_ffi.swift` is compiled into the app target automatically
  (it lives inside the file-system-synchronized source folder).

The generated `.swift` does `import ainm_escrow_ffiFFI` — the low-level FFI module
declared in the xcframework's `module.modulemap`. So both the committed `.swift`
**and** the regenerated `.xcframework` must be present for the app to build.

### Swift API surface

```swift
func escrowEnroll(secret: Data, pin: String) throws -> EnrollResult
EscrowRecovery.begin(pin: String, kdfParamsJson: String) throws -> EscrowRecovery
recovery.blindedPoint() throws -> String
recovery.finalize(evaluatedPoint: String, ciphertextB64: String) throws -> Data
```
