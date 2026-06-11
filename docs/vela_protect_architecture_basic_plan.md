# Vela Protect Architecture and Basic Implementation Plan

> **Track:** Vela native client bytecode protection  
> **Document status:** architecture + basic-version Codex execution plan  
> **Compatibility policy:** breaking changes are allowed where needed. Protection is designed around linked bytecode and the clean identity/linker architecture.  
> **Implementation scope in this document:** only the **basic version** implementation plan is included. Medium/high protection phases are described architecturally but intentionally left without detailed task plans until the language and bytecode format stabilize.

---

## 0. Executive Summary

Vela should provide bytecode protection as a native publishing capability, not as an external post-processing hack.

This capability should be designed as **Vela Protect**, with two related but distinct feature families:

```text
1. Bytecode Obfuscation
   Make linked bytecode harder to read, decompile, grep, and statically analyze.

2. Protected Execution / VM Decryption
   Store code sections in protected/encrypted form and let the VM decode/decrypt them on demand.
```

The basic version should focus on practical client protection without destabilizing the VM:

```text
Basic version:
- dedicated protected bundle format
- export ABI table separate from debug names
- metadata/debug stripping
- private debug sidecar map
- constant string/bytes protection
- bundle integrity/signing hooks
- reflection lockdown
- protected-bundle loading path
```

Medium/high versions can later add:

```text
- function/constant/register layout randomization
- basic block permutation
- opcode remapping
- function-level VM decryption
- block-level VM decryption
- decoded block cache
- stronger key management
```

The most important architectural rule:

```text
Exports are ABI.
Debug names are diagnostics.
Internal symbols are disposable.
Protected bundles should not require internal names to execute.
```

---

## 1. Goals

### 1.1 Primary goals

Vela Protect should:

- Provide native bytecode protection for client-side Vela scripts.
- Make protected bundles harder to statically inspect or casually modify.
- Separate callable public exports from debug/internal symbols.
- Remove source-level semantic metadata from client bundles.
- Protect string and bytes constants from direct extraction by tools such as `strings`.
- Produce a private debug map for crash report symbolication.
- Support reflection lockdown for protected client profiles.
- Define a future-compatible architecture for VM on-demand decryption.
- Preserve normal VM execution semantics.
- Keep basic version implementation small enough to land before language stabilization.

### 1.2 Secondary goals

Vela Protect should eventually support:

- deterministic protected builds with a seed;
- stronger layout randomization;
- control-flow hardening;
- protected delta/update bundles;
- host-provided or session-provided unlock keys;
- function-level or block-level VM decryption;
- no-JIT protected execution profiles.

---

## 2. Non-goals and Security Disclaimer

### 2.1 Non-goals

Vela Protect does **not** guarantee:

- absolute secrecy of code running on a client machine;
- protection of true secrets embedded in client bundles;
- prevention of runtime hooking;
- prevention of memory dumping;
- prevention of patching the local VM;
- server-side security.

Client-side protection is cost-raising, not absolute security.

### 2.2 Threat model

Vela Protect is intended to resist:

```text
casual static reverse engineering
strings/grep extraction
simple bytecode deserialization
simple decompiler output
basic client package modification
unintentional metadata leakage
```

It is not intended to defeat a determined attacker with:

```text
debugger access
VM patching
runtime hooks
memory dump tooling
custom instrumented VM builds
```

Architecture and documentation must be honest about this.

---

## 3. Current Architectural Fit

Vela is already in a good position for native protection because:

- Linked bytecode uses dense handles for native/script/method/type/variant/field references.
- Debug names are already side-table oriented.
- Linked instructions no longer need source names for hot dispatch.
- The linker has already separated semantic identity from runtime operands.
- Runtime images already distinguish program image, linked program, runtime state, hot reload versioning, and persistent heap state.

This means protection should happen after linking:

```text
source
  -> parse / HIR / semantic
  -> unlinked bytecode
  -> linker
  -> linked bytecode
  -> protection pipeline
  -> protected bundle
  -> runtime load / execution
```

Do not implement primary protection at the parser or AST level. The linked bytecode layer is the right boundary.

---

## 4. Terminology

### 4.1 Linked program

The normal executable bytecode image produced by the linker.

It contains:

```text
functions
linked code objects
dense handles
debug names
runtime tables
constants
instructions
```

### 4.2 Protected bundle

A serialized publishing artifact produced from a linked program plus protection options.

Suggested extension:

```text
.vbc
```

### 4.3 Debug map

A private sidecar artifact used to map protected/obfuscated locations back to original source/debug information.

Suggested extension:

```text
.vmap
```

This file must not be shipped to clients.

### 4.4 Export table

A stable public ABI table for host/client calls.

It maps public names such as `"tick"` or `"on_event"` to protected function handles.

Exports must remain callable even when debug names are stripped or remapped.

### 4.5 Internal symbols

Names and metadata not required for public ABI.

Examples:

```text
private function names
local names
parameter names
private type names
private method names
frame slot names
source spans
docs
attrs
```

Internal symbols can be stripped or remapped.

### 4.6 Constant protection

Encoding/encryption/obfuscation of constants such as:

```text
string constants
bytes constants
large static tables
URLs
error strings
protocol constants
feature flags
```

### 4.7 VM protected execution

Runtime support for executing protected instruction sections that are not stored as plaintext linked instructions in the bundle.

This is **not** part of the basic implementation plan, but the architecture must reserve space for it.

---

## 5. Protection Profiles

Protection must not be a single boolean. It should be profile-driven.

### 5.1 `Debug`

Development profile.

```text
- keep debug names
- keep source spans
- keep frame slots
- keep reflection metadata
- keep constants plain
- no signing requirement
- no VM decryption
```

### 5.2 `ReleaseStrip`

Server/release profile without client-hardening.

```text
- strip source spans
- strip frame local names
- strip private debug names
- keep export ABI
- keep constants plain by default
- no VM decryption
```

### 5.3 `ClientLight`

Basic client protection profile.

```text
ReleaseStrip
+ export ABI table
+ internal symbol stripping/remapping
+ private debug map generation
+ string/bytes constant protection
+ reflection lockdown
+ bundle integrity/signing hooks
```

This is the **basic version** target.

### 5.4 `ClientHard` future profile

Medium-level future protection.

```text
ClientLight
+ function table shuffle
+ constant table shuffle
+ register renaming
+ basic block splitting/permutation
+ optional opcode remap
+ limited control-flow hardening
```

Implementation plan intentionally deferred.

### 5.5 `ClientHardEncrypted` future profile

High-level protected execution profile.

```text
ClientHard
+ protected/encrypted instruction sections
+ VM on-demand function/block decryption
+ decoded code cache
+ zeroization on eviction
+ no JIT
```

Implementation plan intentionally deferred.

---

## 6. High-level Architecture

### 6.1 Pipeline

```text
UnlinkedProgram
      ↓
Linker
      ↓
LinkedProgram
      ↓
ProtectionPipeline
      ↓
ProtectedBundle + DebugMap
      ↓
Runtime loader
      ↓
VM execution
```

### 6.2 New conceptual crate

Add a dedicated crate:

```text
crates/vela_protect
```

Responsibilities:

```text
protection profiles
export table generation
metadata stripping
debug map generation
constant protection
protected bundle construction
bundle verification
reflection lockdown metadata
future code section protection
```

`vela_protect` should depend on linked bytecode types, not on parser or AST internals unless needed for source map generation.

### 6.3 Runtime integration

Runtime should be able to load either:

```text
normal linked image
protected bundle image
```

Basic version can decode protected bundle into an executable linked program at load time, with stripped metadata and decoded constants as needed.

Future encrypted execution should avoid whole-program plaintext reconstruction.

---

## 7. Export ABI Table

### 7.1 Why exports must be separate from debug names

Debug names are optional and can be stripped. Public callability cannot depend on them.

A protected client bundle must support:

```rust
runtime.call("tick", args, options)
```

even if internal function debug names have been removed.

### 7.2 Export table shape

Suggested structure:

```rust
pub struct ExportTable {
    pub exports: Vec<ExportEntry>,
}

pub struct ExportEntry {
    pub public_name: String,
    pub function: ScriptFunctionHandle,
    pub arity: ArityInfo,
    pub param_names: ExportParamNames,
    pub param_defaults: Vec<bool>,
}
```

`param_names` policy:

```rust
pub enum ExportParamNames {
    Keep(Vec<String>),
    Strip,
}
```

For basic version, keep export param names only if host APIs need named arguments. Otherwise strip them.

### 7.3 Export declaration sources

Exports may come from:

```text
explicit attributes
CLI build flags
engine/runtime builder config
default entry selection
```

Suggested attribute:

```vela
#[export("tick")]
fn tick(dt: f32) { ... }
```

Basic version can start with CLI/API-provided exports before adding attributes.

### 7.4 Export policy

Default policy for client profiles:

```text
- exported public names are preserved
- exported function internal debug names can still be stripped
- non-exported functions are not callable by public name
```

---

## 8. Metadata Stripping

### 8.1 Metadata categories

Strip or remap:

```text
source spans
source file paths
function debug names
parameter debug names
local/frame slot names
private type names
private method names
private field names
docs
attrs
diagnostic labels with source snippets
```

Keep or transform:

```text
export names
public ABI arity
runtime-required handles
capability requirements
error codes
opaque location ids
```

### 8.2 Debug name replacement

Client profile should replace internal debug names with opaque names:

```text
fn#0001
fn#0002
p#0001
slot#0004
```

or with numeric IDs only.

### 8.3 Source spans

For client bundles:

```text
source spans should not contain source file paths or original ranges
```

Use opaque protected locations:

```rust
pub struct ProtectedLocation {
    pub function_obf_id: u64,
    pub pc: u32,
}
```

### 8.4 Frame metadata

Basic client profile should remove:

```text
local names
temporary names
frame slot binding names
source spans for frame slots
```

Frame metadata may be entirely absent unless debugger/profile support is explicitly requested.

---

## 9. Private Debug Map

### 9.1 Purpose

Protected client bundles need private symbolication.

The debug map maps:

```text
protected function id -> original function name
protected pc -> source span
protected debug name -> original debug name
export entry -> original function
constant id -> optional original metadata
```

### 9.2 Debug map artifact

Suggested extension:

```text
.vmap
```

It must be generated alongside `.vbc` and stored privately.

### 9.3 Debug map contents

Suggested structure:

```rust
pub struct DebugMap {
    pub build_id: BuildId,
    pub source_files: Vec<SourceFileEntry>,
    pub functions: Vec<FunctionDebugMapEntry>,
    pub names: Vec<NameMapEntry>,
    pub locations: Vec<LocationMapEntry>,
}
```

### 9.4 Protected runtime error mapping

Client runtime error:

```text
runtime error vm::type_mismatch at obf 8d31:005d
```

Server/tooling side:

```text
8d31:005d -> scripts/combat.vela:88 in calculate_damage
```

---

## 10. Constant Protection

### 10.1 Target constants

Protect:

```text
string constants
bytes constants
large table constants
URLs
protocol strings
user-facing hidden error text
feature flags
sensitive labels
map keys where appropriate
```

### 10.2 Non-goal

Constant protection does not prevent runtime extraction once a value is used.

It prevents direct static extraction from the bundle.

### 10.3 Protected constant model

Suggested structures:

```rust
pub enum ProtectedConstant {
    Plain(Constant),
    EncodedString(EncodedBlob),
    EncodedBytes(EncodedBlob),
    SplitString(Vec<EncodedBlob>),
}

pub struct EncodedBlob {
    pub algorithm: ConstantProtectionAlgorithm,
    pub nonce: Vec<u8>,
    pub data: Vec<u8>,
    pub tag: Option<Vec<u8>>,
}
```

### 10.4 Algorithms

Basic version can start with a simple reversible transform for obfuscation, but the architecture must allow authenticated encryption later.

Supported modes:

```text
None
XorStreamDerived
AeadEncrypted
```

Basic version target:

```text
XorStreamDerived or simple stream transform + integrity hash
```

Future hard profile:

```text
AEAD per constant
```

### 10.5 Key derivation

Even basic constant protection should avoid one global repeated key.

Use:

```text
constant_key = KDF(image_seed, constant_index, function_id, constant_kind)
```

Key modes:

```text
EmbeddedDerivedKey
HostProvidedKey
SessionKey
```

Basic version can implement:

```text
EmbeddedDerivedKey
```

and reserve the others.

### 10.6 Lazy decode

Basic version may decode at load time for simplicity, but architecture should support lazy decode:

```text
LoadConst -> decode protected constant -> allocate runtime value
```

Future encrypted profiles should prefer lazy decode.

---

## 11. Bytecode Layout Obfuscation

This is medium-level future work, but the architecture should define it now.

### 11.1 Function table shuffle

Reorder function table and update all script function handles.

### 11.2 Constant table shuffle

Reorder constants inside each function and update `ConstantId` references.

### 11.3 Register renaming

Permute registers inside each function while preserving parameter/capture calling conventions.

### 11.4 Basic block permutation

Split into basic blocks, reorder blocks, retarget jumps.

### 11.5 Opcode remap

Encode instructions with a per-build opcode mapping.

This belongs to protected serialization, not to the internal VM enum layout.

### 11.6 Deterministic seed

All randomization should support deterministic reproducibility:

```text
same input + same seed -> same protected bundle
same input + different seed -> different protected layout
```

Implementation plan deferred.

---

## 12. Control-flow Hardening

Control-flow hardening is future work.

Possible features:

```text
dead block insertion
opaque no-op branches
basic block splitting
jump threading
limited control-flow flattening
```

Design constraints:

```text
must be verifier-backed
must have code-size budget
must have performance budget
must be disabled by default for ClientLight
```

Not part of basic implementation.

---

## 13. Protected Bundle Format

### 13.1 Artifact

Suggested extension:

```text
.vbc
```

### 13.2 Sections

Basic version should define sections even if some remain plain:

```text
header
format version
protection profile
feature flags
export table
public ABI metadata
linked runtime tables
protected/stripped function metadata
instruction section
constant section
reflection policy section
capability requirements
integrity manifest
signature section
```

### 13.3 Header

Suggested fields:

```rust
pub struct ProtectedBundleHeader {
    pub magic: [u8; 4],
    pub format_version: u32,
    pub bytecode_version: u32,
    pub protection_profile: ProtectionProfile,
    pub feature_flags: ProtectionFeatureFlags,
    pub build_id: BuildId,
}
```

### 13.4 Feature flags

Examples:

```text
STRIPPED_DEBUG
PROTECTED_CONSTANTS
SIGNED
REFLECTION_LOCKDOWN
ENCRYPTED_CODE_SECTIONS
OPCODE_REMAP
BLOCK_DECRYPTION
```

Basic version should use:

```text
STRIPPED_DEBUG
PROTECTED_CONSTANTS
SIGNED or HASHED
REFLECTION_LOCKDOWN
```

---

## 14. Bundle Integrity and Signing

### 14.1 Integrity

Basic version must include at least:

```text
section hashes
bundle manifest hash
load-time verification
```

### 14.2 Signing hooks

Do not hardwire one signing provider into core architecture.

Define traits:

```rust
pub trait BundleSigner {
    fn sign(&self, payload: &[u8]) -> ProtectionResult<Signature>;
}

pub trait BundleVerifier {
    fn verify(&self, payload: &[u8], signature: &Signature) -> ProtectionResult<()>;
}
```

Basic version can include:

```text
unsigned hash-only mode for tests
detached signature shape
embedded signature shape
```

Actual production signing provider can be added later.

### 14.3 Tamper behavior

If integrity verification fails:

```text
bundle load fails
VM must not run partially verified bytecode
```

---

## 15. Reflection Lockdown

### 15.1 Policy

Add reflection lockdown policy:

```rust
pub enum ProtectedReflectionPolicy {
    KeepAll,
    ExportedOnly,
    PublicSchemaOnly,
    Disabled,
}
```

### 15.2 Basic client default

For `ClientLight`:

```text
script internals: disabled
exports: visible by public name and arity only
stdlib: minimal
host public schema: policy-driven
docs/attrs/source spans: stripped
```

### 15.3 Reflection must not bypass obfuscation

Protected bundles must not allow script reflection to recover:

```text
internal function names
private type names
private field names
local names
source spans
docs
attrs
```

---

## 16. Error Redaction

### 16.1 Error policy by profile

```text
Debug:
  full diagnostics

ReleaseStrip:
  error code + maybe public function context

ClientLight:
  error code + opaque protected location

ClientHard / ClientHardEncrypted:
  error code + opaque location only
```

### 16.2 Redacted error shape

```rust
pub struct ProtectedErrorLocation {
    pub build_id: BuildId,
    pub function_obf_id: u64,
    pub pc: u32,
}
```

### 16.3 Sidecar symbolication

Use `.vmap` to restore full diagnostics outside the client.

---

## 17. VM Protected Execution / Decryption Architecture

This section defines the complete architecture, but **not the basic implementation plan**.

### 17.1 Purpose

VM protected execution means the bundle does not store executable instruction sections in plaintext linked-instruction form.

The VM decodes/decrypts function or block sections on demand.

### 17.2 Execution modes

```rust
pub enum CodeStorageMode {
    Plain,
    ProtectedFunctionLevel,
    ProtectedBlockLevel,
}
```

### 17.3 Function-level decrypt

Behavior:

```text
enter function
decrypt/authenticate whole function body
execute from decoded function cache
zeroize on cache eviction
```

Pros:

```text
simpler
faster
good first encrypted-execution milestone
```

Cons:

```text
whole function exists plaintext in memory while cached
```

### 17.4 Block-level decrypt

Behavior:

```text
split function into basic blocks
encrypt/authenticate each block
VM decrypts target block on demand
decoded block cache holds recent blocks
zeroize on eviction
```

Pros:

```text
smaller plaintext window
better for ClientHardEncrypted
```

Cons:

```text
more complex
jump/cache/error mapping complexity
```

### 17.5 No per-instruction decrypt

Do not design per-instruction decrypt as the main architecture.

Problems:

```text
high overhead
complex fetch/decode path
limited security gain
bad for profiling
bad for future async/safepoints
```

### 17.6 Instruction provider abstraction

VM fetch should eventually go through:

```rust
pub trait InstructionProvider {
    fn fetch(
        &mut self,
        function: ScriptFunctionHandle,
        ip: InstructionOffset,
    ) -> VmResult<FetchedInstruction>;
}
```

Providers:

```text
PlainInstructionProvider
ProtectedFunctionInstructionProvider
ProtectedBlockInstructionProvider
```

VM state should store logical IP, never plaintext instruction pointers.

### 17.7 Encrypted code section metadata

Protected functions/blocks need:

```rust
pub struct EncryptedBlock {
    pub block_id: BlockId,
    pub logical_start_ip: u32,
    pub instruction_count: u32,
    pub ciphertext_offset: u32,
    pub ciphertext_len: u32,
    pub nonce: Vec<u8>,
    pub tag: Vec<u8>,
}
```

### 17.8 Authentication

Encrypted code must be authenticated.

Authentication failure:

```text
fatal protected bytecode violation
no fallback
no partial execution
```

### 17.9 Associated data

Use associated data such as:

```text
bundle id
format version
profile
function handle/id
block id
logical start ip
instruction count
section hash
```

This prevents block swapping.

---

## 18. Key Management Modes

Key management is part of the architecture even if only one mode is implemented initially.

```rust
pub enum ProtectionKeyMode {
    None,
    EmbeddedDerivedKey,
    HostProvidedKey,
    SessionKey,
    SplitKey,
}
```

### 18.1 `None`

No encryption; obfuscation/integrity only.

### 18.2 `EmbeddedDerivedKey`

Bundle/VM-derived key material.

```text
works offline
raises static reverse-engineering cost
does not protect true secrets
```

Basic version can use this for constant protection.

### 18.3 `HostProvidedKey`

Host supplies unlock key at load time.

```rust
runtime.load_protected_bundle(bundle, UnlockKey::from_bytes(...))
```

Future work.

### 18.4 `SessionKey`

Key acquired from server/auth/session flow.

Future work.

### 18.5 `SplitKey`

Part bundle-derived, part host/server/device-derived.

Future work.

---

## 19. Decoded Cache and Zeroization

Future VM decryption requires decoded cache management.

### 19.1 Cache types

```text
DecodedFunctionCache
DecodedBlockCache
```

### 19.2 Cache policy

Configurable:

```text
max decoded functions
max decoded blocks
max decoded bytes
clear on memory pressure
clear on protected update
optional clear on await/call boundary
```

### 19.3 Cache key

```rust
pub struct DecodedCacheKey {
    pub build_id: BuildId,
    pub program_version: Option<ProgramVersionId>,
    pub function: ScriptFunctionHandle,
    pub block: Option<BlockId>,
}
```

### 19.4 Zeroization

When evicting:

```text
zero decoded instruction bytes
zero decoded constant temporary buffers
do not log plaintext
```

Use a zeroization crate or explicit secure clearing where appropriate.

---

## 20. Protected Verifier

### 20.1 Plain verifier

Run normal linked verifier before protection.

### 20.2 Protected verifier

At bundle load time verify:

```text
header
section bounds
section hashes
export table target validity
reflection policy consistency
constant protection metadata
signature/integrity
encrypted code metadata if present
```

### 20.3 Lazy decoded verifier

Future encrypted execution can verify decoded function/block after decrypt.

Modes:

```text
eager verify all decoded sections
lazy verify on first use
debug/CI force eager verification
```

Basic version should only need protected-bundle structural verification.

---

## 21. Protected Update / Hot Reload Policy

### 21.1 Development

```text
Debug / ReleaseStrip:
  normal hot reload allowed
```

### 21.2 Client profiles

```text
ClientLight / ClientHard:
  signed protected update only
```

### 21.3 Encrypted profiles

```text
ClientHardEncrypted:
  signed protected update
  matching key policy
  versioned decoded cache invalidation
```

### 21.4 Running frames

Running or suspended frames should pin the program version they started with.

New calls can use the new version after update.

---

## 22. Runtime and CLI Surface

### 22.1 Rust API concepts

```rust
pub enum ProtectionProfile { ... }

pub struct ProtectionOptions { ... }

pub struct ProtectedBundle { ... }

pub struct DebugMap { ... }

pub struct ExportTable { ... }

pub enum ProtectionKeyMode { ... }

pub struct ProtectedRuntimePolicy { ... }
```

### 22.2 Build API

```rust
let linked = engine.link_program(&program)?;

let output = VelaProtector::new()
    .profile(ProtectionProfile::ClientLight)
    .export("tick")
    .export("on_event")
    .protect(linked)?;

output.write_bundle("game.vbc")?;
output.write_debug_map("game.vmap")?;
```

### 22.3 Runtime load API

```rust
let bundle = ProtectedBundle::read("game.vbc")?;
let runtime = Runtime::from_protected_bundle(engine, bundle)?;
```

Future encrypted load:

```rust
let runtime = Runtime::from_protected_bundle_with_key(engine, bundle, unlock_key)?;
```

### 22.4 CLI

```bash
vela build game.vela \
  --protect client-light \
  --export tick \
  --export on_event \
  --out game.vbc \
  --debug-map game.vmap
```

Inspection:

```bash
vela inspect game.vbc --public
vela verify game.vbc
vela symbolize game.vmap --location 8d31:005d
```

---

## 23. Basic Version Scope

The basic version implements **ClientLight** only.

### 23.1 Included

```text
vela_protect crate
ProtectionProfile::ClientLight
ExportTable
ProtectedBundle structure
DebugMap sidecar
metadata stripping
debug name remapping/stripping
string/bytes constant protection
bundle integrity hash
signing trait shape
reflection lockdown metadata
protected-bundle loader
basic CLI/API hooks
```

### 23.2 Not included

```text
function table shuffle
constant table shuffle
register renaming
basic block permutation
opcode remap
control-flow hardening
VM function-level decrypt
VM block-level decrypt
decoded block cache
host/session key modes
anti-debug features
JIT integration
```

These are architecture-defined but not planned for basic implementation.

---

# 24. Basic Implementation Plan for Codex

## Phase 0: Architecture documentation

### Task 0.1: Add Vela Protect architecture doc

- [ ] Add `docs/architecture/vela-protect.md`.
- [ ] Include threat model and non-goals.
- [ ] Define obfuscation vs protected execution as separate feature families.
- [ ] Define protection profiles.
- [ ] Define basic version scope.
- [ ] Define future medium/high profiles without task plans.
- [ ] Mention that ClientHardEncrypted disables JIT.

**Termination condition:**

- [ ] Architecture document exists.
- [ ] Basic version scope is clearly separated from future work.
- [ ] No implementation change required.

---

## Phase 1: New crate and core types

### Task 1.1: Add `vela_protect` crate

- [ ] Add `crates/vela_protect`.
- [ ] Add it to workspace.
- [ ] Add dependency on `vela_bytecode`, `vela_common`, and other needed internal crates.
- [ ] Define crate modules:
  - [ ] `profile`
  - [ ] `export`
  - [ ] `bundle`
  - [ ] `debug_map`
  - [ ] `strip`
  - [ ] `constants`
  - [ ] `integrity`
  - [ ] `error`

**Termination condition:**

- [ ] `cargo test -p vela_protect` compiles with empty/basic tests.
- [ ] Workspace compiles.

### Task 1.2: Define protection profiles and options

- [ ] Add `ProtectionProfile`.
- [ ] Add `ProtectionOptions`.
- [ ] Add `ProtectedReflectionPolicy`.
- [ ] Add `ProtectionFeatureFlags`.
- [ ] Add default options for:
  - [ ] `Debug`
  - [ ] `ReleaseStrip`
  - [ ] `ClientLight`

**Termination condition:**

- [ ] Unit tests assert expected default options for `ClientLight`.
- [ ] `ClientLight` enables stripping, constant protection, reflection lockdown, and integrity.

---

## Phase 2: Export ABI table

### Task 2.1: Add export table types

- [ ] Add `ExportTable`.
- [ ] Add `ExportEntry`.
- [ ] Add `ArityInfo`.
- [ ] Add public export lookup by name.
- [ ] Add validation for duplicate export names.

**Termination condition:**

- [ ] Duplicate export names are rejected.
- [ ] Export lookup returns the correct script function handle.

### Task 2.2: Build export table from linked program and options

- [ ] Add `ProtectionOptions::exports`.
- [ ] Build export table from explicit requested names.
- [ ] Resolve export names against linked program entry points for basic version.
- [ ] Preserve export public names even when debug names are stripped.

**Termination condition:**

- [ ] Protected output can call exported `main`/`tick` by public name.
- [ ] Internal functions are not public exports unless requested.

---

## Phase 3: Debug stripping and debug map

### Task 3.1: Add debug map types

- [ ] Add `DebugMap`.
- [ ] Add `FunctionDebugMapEntry`.
- [ ] Add `LocationMapEntry`.
- [ ] Add `NameMapEntry`.
- [ ] Add build id support.

**Termination condition:**

- [ ] Debug map can record original function/debug names before stripping.
- [ ] Debug map can map protected location to original location placeholder.

### Task 3.2: Add metadata stripping pass

- [ ] Strip source spans from linked code.
- [ ] Strip or remap internal function debug names.
- [ ] Strip parameter names for non-exported functions.
- [ ] Strip frame slot names and spans.
- [ ] Preserve export table public names.
- [ ] Record stripped names/spans into debug map.

**Termination condition:**

- [ ] Protected linked image still verifies/runs.
- [ ] Internal function/local names are absent from protected bundle representation.
- [ ] Debug map contains enough data to restore names offline.

### Task 3.3: Add redacted diagnostic location support

- [ ] Define `ProtectedLocation`.
- [ ] Ensure protected code can report opaque function/pc location.
- [ ] Do not expose original source file/span in client profile.

**Termination condition:**

- [ ] ClientLight runtime errors expose error code + opaque location, not source path or internal function name.

---

## Phase 4: Constant protection

### Task 4.1: Add protected constant representation

- [ ] Add `ProtectedConstant`.
- [ ] Add `EncodedBlob`.
- [ ] Add `ConstantProtectionAlgorithm`.
- [ ] Support at least string and bytes constants.
- [ ] Allow constants to remain plain through policy.

**Termination condition:**

- [ ] Unit tests encode/decode string and bytes constants.

### Task 4.2: Implement basic constant transform

- [ ] Add per-image seed.
- [ ] Add deterministic per-constant derived stream.
- [ ] Encode string constants.
- [ ] Encode bytes constants.
- [ ] Decode back to original runtime constants.
- [ ] Avoid storing protected strings as plaintext in serialized bundle.

**Termination condition:**

- [ ] Protected bundle bytes do not contain protected string constants in plaintext.
- [ ] Runtime result is identical to unprotected execution for protected constants.

### Task 4.3: Integrate protected constants into bundle

- [ ] Store protected constant section in `ProtectedBundle`.
- [ ] Add load-time decode path for basic version.
- [ ] Preserve architecture for future lazy decode.
- [ ] Add tests for multiple functions and duplicate constants.

**Termination condition:**

- [ ] Protected bundle loads and runs programs using string and bytes constants.
- [ ] Protected constant section has deterministic output for same seed.

---

## Phase 5: Protected bundle format and integrity

### Task 5.1: Define protected bundle structures

- [ ] Add `ProtectedBundle`.
- [ ] Add `ProtectedBundleHeader`.
- [ ] Add section table model.
- [ ] Add profile/feature flags to header.
- [ ] Add export table section.
- [ ] Add protected program section.
- [ ] Add protected constant section.
- [ ] Add reflection policy section.

**Termination condition:**

- [ ] Bundle can be constructed in memory from protected linked program.
- [ ] Bundle records profile and feature flags.

### Task 5.2: Add serialization boundary

- [ ] Add encode/decode APIs for `ProtectedBundle`.
- [ ] Do not rely on Rust enum memory layout.
- [ ] Keep format versioned.
- [ ] Add round-trip tests.

**Termination condition:**

- [ ] `ProtectedBundle::encode` then `decode` round-trips.
- [ ] Unsupported format version is rejected.

### Task 5.3: Add integrity manifest

- [ ] Add section hashes.
- [ ] Add bundle manifest hash.
- [ ] Verify hashes on decode/load.
- [ ] Add tamper tests.

**Termination condition:**

- [ ] Modified bundle bytes fail verification.
- [ ] Valid bundle loads.

### Task 5.4: Add signing trait shape

- [ ] Add `BundleSigner` trait.
- [ ] Add `BundleVerifier` trait.
- [ ] Add detached/embedded signature fields.
- [ ] Add test dummy signer/verifier.

**Termination condition:**

- [ ] Valid dummy signature passes.
- [ ] Invalid dummy signature fails.
- [ ] ClientLight can require signature through policy, even if production signer is not implemented yet.

---

## Phase 6: Reflection lockdown metadata

### Task 6.1: Add protected reflection policy section

- [ ] Encode `ProtectedReflectionPolicy` into bundle.
- [ ] Add policy to runtime image/load context.
- [ ] Ensure ClientLight defaults to internal script reflection lockdown.

**Termination condition:**

- [ ] Protected bundle records reflection policy.
- [ ] ClientLight policy strips/locks internal script reflection metadata.

### Task 6.2: Runtime reflection enforcement

- [ ] Ensure reflection APIs consult protected policy.
- [ ] Disable internal script function/type/field enumeration for ClientLight.
- [ ] Preserve public export ABI reflection if enabled.
- [ ] Preserve stdlib/host reflection only according to policy.

**Termination condition:**

- [ ] Reflection cannot recover internal names from ClientLight bundle.
- [ ] Exported functions remain discoverable only through export ABI metadata.

---

## Phase 7: Runtime and engine integration

### Task 7.1: Add protected image loading path

- [ ] Add engine/runtime API to create runtime from `ProtectedBundle`.
- [ ] Verify integrity before execution.
- [ ] Decode/load protected program for basic version.
- [ ] Attach export table to runtime image.

**Termination condition:**

- [ ] Runtime can call exported function from protected bundle.
- [ ] Runtime refuses invalid/tampered bundle.

### Task 7.2: Separate export lookup from debug-name lookup

- [ ] Runtime call path should support export table lookup.
- [ ] Do not require debug names for protected calls.
- [ ] Existing unprotected debug-name entry lookup can remain for debug/unprotected profiles.

**Termination condition:**

- [ ] Protected bundle with stripped debug names can still call `"tick"` via export table.
- [ ] Internal function names are not callable unless exported.

### Task 7.3: Add CLI/API hooks

- [ ] Add build API in engine/protect layer.
- [ ] Add CLI flags:
  - [ ] `--protect client-light`
  - [ ] `--export <name>`
  - [ ] `--out <file.vbc>`
  - [ ] `--debug-map <file.vmap>`
- [ ] Add verify/inspect commands if CLI scope allows.

**Termination condition:**

- [ ] CLI can produce `.vbc` and `.vmap`.
- [ ] CLI can verify `.vbc`.
- [ ] Protected `.vbc` can be loaded and run in tests.

---

## Phase 8: Basic protected profile validation

### Task 8.1: Add positive conformance tests

- [ ] Protected program with exported `main` runs.
- [ ] Protected program with internal helper runs.
- [ ] Protected program with string constants runs.
- [ ] Protected program with bytes constants runs.
- [ ] Protected program with record/enum usage runs.
- [ ] Protected program with stdlib calls runs.

**Termination condition:**

- [ ] Protected and unprotected outputs match.

### Task 8.2: Add negative/security-shape tests

- [ ] Internal function names absent from bundle.
- [ ] Local names absent from bundle.
- [ ] Source file paths absent from bundle.
- [ ] Protected string constants absent in plaintext.
- [ ] Tampered bundle fails verification.
- [ ] Reflection cannot enumerate internal script symbols.
- [ ] Non-exported function cannot be called by name.

**Termination condition:**

- [ ] All negative tests pass.
- [ ] Grep/string scan tests confirm expected stripping/protection.

### Task 8.3: Add reproducibility tests

- [ ] Same source + same protection seed produces same bundle.
- [ ] Same source + different seed changes protected constants/debug remaps where applicable.
- [ ] Debug map build id matches bundle build id.

**Termination condition:**

- [ ] Reproducible build tests pass.

---

## Phase 9: Documentation and safety notes

### Task 9.1: User-facing documentation

- [ ] Document `Debug`, `ReleaseStrip`, `ClientLight`.
- [ ] Document what ClientLight protects.
- [ ] Document what ClientLight does not protect.
- [ ] Document export table usage.
- [ ] Document debug map storage requirements.
- [ ] Document reflection lockdown behavior.

**Termination condition:**

- [ ] Users can understand how to build and run a protected bundle.
- [ ] Security disclaimer is explicit.

### Task 9.2: Internal developer documentation

- [ ] Document bundle format.
- [ ] Document protection pipeline.
- [ ] Document test strategy.
- [ ] Document future profiles but mark them as deferred.

**Termination condition:**

- [ ] Future ClientHard/ClientHardEncrypted work has clear architectural anchors.

---

# 25. Basic Version Final Termination Criteria

The basic protection feature is complete when:

- [ ] `vela_protect` crate exists.
- [ ] `ProtectionProfile::ClientLight` exists.
- [ ] Protected bundle format exists.
- [ ] Export table exists and is independent of debug names.
- [ ] Runtime can call exported functions from protected bundles.
- [ ] Internal debug names can be stripped/remapped.
- [ ] Source spans and frame local names are stripped for ClientLight.
- [ ] Private debug map is generated.
- [ ] String constants can be protected.
- [ ] Bytes constants can be protected.
- [ ] Bundle integrity verification exists.
- [ ] Signing trait shape exists.
- [ ] Reflection lockdown policy exists.
- [ ] ClientLight reflection cannot recover internal symbols.
- [ ] Protected bundle can be serialized/deserialized.
- [ ] Tampered protected bundle fails verification.
- [ ] Protected and unprotected program behavior matches.
- [ ] Documentation includes threat model and non-goals.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.

---

## 26. Future Work Placeholder

Do not implement these in the basic version:

```text
ClientHard
ClientHardEncrypted
layout randomization
control-flow hardening
function-level VM decryption
block-level VM decryption
decoded block cache
host/session key management
opcode remapping
protected update bundles
```

But keep the architecture compatible with them by ensuring:

```text
export table is separate from debug names
bundle format is sectioned and versioned
protected constants are separate from normal constants
protected runtime policy is explicit
reflection lockdown is profile-driven
instruction storage is not assumed to always be plain forever
```

---

## 27. Short Codex Summary

Use this as the short guiding rule for Codex tasks:

```text
Build Vela Protect basic version around ClientLight.
Do not implement VM decryption yet.
Do not implement control-flow hardening yet.
Do not depend on debug names for public calls.
Create ExportTable, ProtectedBundle, DebugMap, metadata stripping, constant protection, integrity verification, and reflection lockdown.
Protected bundles must run the same as linked bytecode while hiding internal names and protected constants from static inspection.
```
