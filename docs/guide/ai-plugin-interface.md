# PicAiPic AI Plugin Interface

PicAiPic is the core photo album app. It owns browsing, library management,
metadata, lightweight editing, import/export, and the user workflow. AI features
are installed as separate plugins. Each plugin can wrap one open-source project,
one model family, one local service, or one command-line tool.

This contract is designed for Windows x64 first, but it keeps platform and
runtime fields explicit so Linux, macOS, CPU-only, CUDA, ROCm, DirectML, and
ONNX-based plugins can be added without changing PicAiPic's core model.

## Principles

- PicAiPic must run normally when no AI plugins are installed.
- Built-in lightweight features stay in PicAiPic and are not rewritten as part
  of plugin support.
- Heavy AI dependencies must be declared by plugins, but runtime environments
  should be reusable when possible. Prefer external or shared runtimes for large
  Python stacks such as PyTorch/CUDA/ROCm; use plugin-private environments only
  when isolation is required.
- A plugin should adapt an upstream open-source project instead of copying that
  project into PicAiPic.
- Plugins never edit source photos in place. They produce new outputs that
  PicAiPic can preview, save, or import into the current album.
- The default trust boundary is local machine only. Remote APIs must be declared
  and explicitly enabled by the user.

## Plugin Granularity

Use one plugin per independent upstream capability whenever practical.

Good plugin boundaries:

```text
picai-nafnet-restore       -> NAFNet denoise/deblur/jpeg restoration
picai-salut-color          -> SA-LUT color transfer and LUT export
picai-mobile-sam-segment   -> MobileSAM segmentation masks
picai-iopaint-inpaint      -> IOPaint/Lama inpainting
picai-iat-exposure         -> Illumination-Adaptive Transformer exposure fix
picai-gpupixel-filter      -> GPUPixel beauty/filter pipeline
picai-banana-color         -> OpenAI-compatible vision color recipe
```

A development project may contain several abilities during prototyping, but the
PicAiPic contract should treat them as independent plugin packages when they are
installed by users.

## Directory Layout

Installed plugins should live under PicAiPic app data by default:

```text
%LOCALAPPDATA%\PicAiPic\plugins\
  picai-nafnet-restore\
    picaipic.plugin.json
    install.bat
    start.bat
    stop.bat
    backend\
    models\
    logs\
```

Development plugins can be registered from any local directory:

```text
D:\ailab\NAFNet\picaipic.plugin.json
D:\ailab\IOPaint-main\picaipic.plugin.json
D:\ailab\PicAiPic-plugins\picai-salut-color\picaipic.plugin.json
```

PicAiPic should store registered plugin paths in app configuration, not inside
the photo library database.

## Core Integration Points

The current PicAiPic codebase already has strong photo-management primitives.
Plugins should integrate through these primitives instead of duplicating library
logic.

Core responsibilities that stay inside PicAiPic:

```text
Album and folder authorization
Library database writes
Import and copy conflict handling
Thumbnail generation and cache invalidation
Preview serving through PicAiPic custom protocols
Metadata refresh after filesystem changes
UI events for library refresh
```

Plugin outputs should come back to PicAiPic as files. PicAiPic decides whether
to copy them into the current folder, index them in place, generate thumbnails,
refresh counts, and update the visible grid.

This matches existing core command boundaries:

```text
import_file       -> copy an external result into an album folder and add it to DB
add_file_to_db    -> register a file already written inside an album folder
update_file_info  -> refresh DB metadata for an existing file
get_file_thumbs   -> fetch or schedule thumbnail generation
```

Plugins should not write directly to the PicAiPic SQLite database.

## Discovery And Registry

PicAiPic should discover plugins from three sources:

```text
%LOCALAPPDATA%\PicAiPic\plugins\*
%PROGRAMDATA%\PicAiPic\plugins\*
User-registered development paths
```

Discovery only reads `picaipic.plugin.json`. It should not start plugin
processes or import plugin code.

PicAiPic should store a plugin registry entry per discovered plugin:

```json
{
  "id": "picai-nafnet-restore",
  "path": "D:\\ailab\\NAFNet",
  "enabled": true,
  "manifestHash": "sha256:...",
  "firstSeenAt": "2026-06-18T12:00:00Z",
  "lastSeenAt": "2026-06-18T12:00:00Z"
}
```

If two plugins declare the same `id`, PicAiPic should disable both until the
user chooses one. Silent replacement is not allowed.

Manifest validation should check:

- Valid JSON.
- Supported `schemaVersion`.
- Unique plugin `id`.
- Supported current platform.
- Valid runtime kind.
- Valid capability IDs and kinds.
- No path traversal in relative commands or model paths.
- Local HTTP URLs bind to loopback by default.

## Manifest

Every plugin has a `picaipic.plugin.json` file at its root.

```json
{
  "schemaVersion": 1,
  "id": "picai-nafnet-restore",
  "name": "NAFNet Restore",
  "version": "0.1.0",
  "publisher": "local",
  "homepage": "https://github.com/megvii-research/NAFNet",
  "license": "Apache-2.0",
  "platforms": ["windows-x64"],
  "compatibility": {
    "minPicAiPicVersion": "0.1.0",
    "pluginApi": "^1.0.0"
  },
  "permissions": {
    "readSelectedFiles": true,
    "writeOutputDir": true,
    "network": {
      "runtime": false,
      "setupDownloads": false,
      "uploadSelectedFiles": false,
      "uploadOutputs": false,
      "allowedDomains": []
    }
  },
  "runtimes": ["python", "local-http"],
  "hardware": {
    "cpu": true,
    "cuda": true,
    "rocm": true,
    "directml": false
  },
  "entry": {
    "kind": "local-http",
    "baseUrl": "http://127.0.0.1:8011",
    "startCommand": "start.bat",
    "stopCommand": "stop.bat",
    "health": {
      "method": "GET",
      "path": "/health",
      "readyField": "ready"
    }
  },
  "install": {
    "kind": "script",
    "command": "install.bat"
  },
  "models": [
    {
      "id": "nafnet-sidd",
      "name": "NAFNet SIDD",
      "required": false,
      "path": "models/NAFNet-SIDD-width64.pth",
      "purpose": "denoise"
    }
  ],
  "capabilities": [
    {
      "id": "denoise",
      "kind": "image.restore.denoise",
      "name": "Denoise",
      "version": "1.0",
      "inputs": [
        { "id": "source", "kind": "image", "required": true }
      ],
      "outputs": [
        { "id": "result", "kind": "image", "required": true }
      ],
      "parameters": {
        "type": "object",
        "properties": {
          "method": {
            "type": "string",
            "enum": ["auto", "model", "fallback"],
            "default": "auto"
          },
          "strength": {
            "type": "number",
            "minimum": 0,
            "maximum": 1,
            "default": 0.55
          }
        }
      },
      "invoke": {
        "method": "POST",
        "path": "/invoke/denoise",
        "contentType": "application/json"
      }
    }
  ]
}
```

## Manifest Fields

`schemaVersion`
: Manifest format version. Current version is `1`.

`id`
: Stable plugin identifier. Use lowercase letters, numbers, dots, and hyphens.

`name`
: Human-readable display name.

`version`
: Plugin package version, not necessarily the upstream model version.

`publisher`
: Author, organization, or `local`.

`homepage`
: Optional upstream or plugin homepage.

`license`
: Plugin or upstream license identifier. Use SPDX when possible.

`platforms`
: Supported package targets. Initial target is `windows-x64`.

`compatibility`
: Optional PicAiPic and plugin API version requirements.

`permissions`
: Optional declaration of what the plugin needs to access.

`runtimes`
: Runtime tags such as `python`, `local-http`, `local-command`, `onnx`,
  `node`, `cuda`, `rocm`, or `directml`.

`hardware`
: Declares supported compute backends. This is informational in version 1.

`entry`
: How PicAiPic starts and communicates with the plugin.

`install`
: Optional installer command. PicAiPic may expose this in a plugin manager.

`models`
: Optional model files or model families used by the plugin.

`capabilities`
: User-facing AI operations exposed by the plugin.

## Version Compatibility

`schemaVersion` describes the manifest file format. `pluginApi` describes the
runtime contract between PicAiPic and the plugin.

Recommended compatibility field:

```json
{
  "compatibility": {
    "minPicAiPicVersion": "0.1.0",
    "maxPicAiPicVersion": null,
    "pluginApi": "^1.0.0"
  }
}
```

Rules:

- PicAiPic should reject unsupported manifest `schemaVersion`.
- PicAiPic should warn when `minPicAiPicVersion` is newer than the current app.
- Minor `pluginApi` updates should remain backward compatible.
- Breaking protocol changes require a new major `pluginApi`.
- Unknown optional fields should be ignored, not treated as fatal errors.
- Unknown required fields must be listed under `requires`.

Example:

```json
{
  "requires": ["permissions.network"]
}
```

If PicAiPic does not understand a required field, it should not enable the
plugin.

## Permissions

Plugins must declare sensitive access. PicAiPic should show these declarations
before the user enables the plugin.

```json
{
  "permissions": {
    "readSelectedFiles": true,
    "readAlbumFolders": false,
    "writeOutputDir": true,
    "writeSourceFiles": false,
    "network": false,
    "launchChildProcesses": true
  }
}
```

Version 1 permissions:

`readSelectedFiles`
: Plugin can read files explicitly sent in a task.

`readAlbumFolders`
: Plugin wants folder-level access beyond selected files.

`writeOutputDir`
: Plugin can write generated files into PicAiPic-provided output directories.

`writeSourceFiles`
: Plugin wants to edit originals. PicAiPic should reject this by default.

`network`
: Plugin uses network access.

`launchChildProcesses`
: Plugin starts helper processes, such as Python workers or model servers.

PicAiPic should pass selected file paths per task instead of giving broad album
access. Broad access should require a clear user action.

## Host-Mediated Actions

Plugins can request powerful actions, but PicAiPic must remain the authority
that decides whether those actions are allowed and how they are executed.

For example, a ChatGPT API based plugin may analyze selected photos and suggest
that some files should be removed, tagged, rated, or moved. The plugin should
return an action proposal, not directly mutate the photo library.

Example action proposal:

```json
{
  "actions": [
    {
      "kind": "photo.trash.move",
      "fileId": 42,
      "confidence": 0.91,
      "reason": "Image is severely blurred and appears to be an accidental shot."
    },
    {
      "kind": "photo.tag.add",
      "fileId": 43,
      "tag": "receipt",
      "confidence": 0.86,
      "reason": "The image contains a printed store receipt."
    }
  ]
}
```

PicAiPic should validate action proposals before execution:

- Check the plugin's declared permissions.
- Check that the target files still belong to the current library scope.
- Show high-risk actions to the user before execution.
- Execute the action through existing PicAiPic commands and database paths.
- Record which plugin requested the action, the reason, and the final result.

Suggested action permission names:

```text
photo.read
photo.metadata.read
photo.metadata.write
photo.tag.write
photo.rating.write
photo.edit.createCopy
photo.trash.suggest
photo.trash.move
photo.delete.permanent
```

`photo.delete.permanent` is reserved and should not be granted to AI plugins in
early versions. Plugins may request trash or cleanup workflows, but permanent
deletion must stay unavailable unless a future PicAiPic release adds a separate
manual-only policy for it.

## Deletion And Trash Policy

AI plugins must not permanently delete files.

Deletion-like actions should use PicAiPic's trash flow:

1. Plugin returns a delete or cleanup proposal.
2. PicAiPic presents a review list with thumbnails, reasons, and confidence.
3. User confirms, edits, or rejects the proposal.
4. PicAiPic moves accepted files to the PicAiPic or system recycle bin.
5. PicAiPic writes an audit record.

The plugin process should never receive a command that lets it unlink arbitrary
paths from disk. This keeps the dangerous operation inside the trusted core and
preserves recovery options after an incorrect AI decision.

## Cloud API Plugins

A plugin may use remote APIs, including ChatGPT or other OpenAI-compatible
services, as long as it declares that behavior clearly.

Cloud API plugins should declare:

```json
{
  "permissions": {
    "readSelectedFiles": true,
    "network": true,
    "remoteApi": true,
    "sendImagesToCloud": true,
    "sendMetadataToCloud": true,
    "manageOwnApiKey": true
  },
  "network": {
    "domains": ["api.openai.com"],
    "purpose": "Vision analysis, tagging, cleanup suggestions, and assistant actions"
  }
}
```

Rules for cloud API plugins:

- PicAiPic should show a privacy notice before enabling the plugin.
- The manifest must state whether original images, thumbnails, filenames,
  EXIF, tags, ratings, or comments may be sent to a remote service.
- API keys must not be stored in `picaipic.plugin.json`.
- API keys should be stored by the plugin in its own secure storage, or by a
  future PicAiPic credential store.
- Logs and diagnostics must redact API keys, tokens, request headers, and user
  secrets.
- Remote API plugins still use host-mediated actions for library changes.

## Tauri Permissions And Plugin UI

PicAiPic is a Tauri app with a fixed capability surface for its own windows.
Plugins should not get direct Tauri permissions, invoke internal Tauri commands,
or access custom protocols as trusted app code.

Rules:

- Plugin UI should be rendered by PicAiPic from manifest-provided parameter
  schemas whenever possible.
- If a plugin provides its own web UI, PicAiPic should treat it as an external
  local web page with no Tauri API access.
- Plugin web UIs must call their own local service, not PicAiPic internal
  commands.
- PicAiPic should mediate file selection, output directories, imports, and
  database writes.
- Do not add plugin windows to the default Tauri capability list by default.

This keeps third-party plugin frontends from inheriting filesystem, shell,
dialog, process, or updater permissions that belong to the core app.

## Runtime Kinds

Version 1 defines two runtime kinds.

### local-http

PicAiPic starts a local service and calls HTTP endpoints on `127.0.0.1`.

Use this for Python/FastAPI, Flask, Node, Rust, C#, or any service process.

Required fields:

```json
{
  "kind": "local-http",
  "baseUrl": "http://127.0.0.1:8011",
  "startCommand": "start.bat",
  "health": {
    "method": "GET",
    "path": "/health",
    "readyField": "ready"
  }
}
```

### local-command

PicAiPic runs a command for each task and reads an output manifest.

Use this for command-line tools that do not need a resident service.

```json
{
  "kind": "local-command",
  "command": "run.bat",
  "protocol": "task-json"
}
```

For `task-json`, PicAiPic writes a task JSON file and passes its path as the
first argument:

```text
run.bat D:\Temp\picaipic-task-123.json
```

## Installation, Update, And Removal

Plugins can be installed manually or by a future PicAiPic plugin manager.

Recommended install metadata:

```json
{
  "install": {
    "kind": "script",
    "command": "install.bat",
    "estimatedDiskMb": 12000,
    "requiresAdmin": false
  },
  "update": {
    "kind": "manual",
    "homepage": "https://github.com/megvii-research/NAFNet"
  },
  "uninstall": {
    "kind": "script",
    "command": "uninstall.bat",
    "preserveModels": true
  }
}
```

Rules:

- The main PicAiPic installer should not install large AI dependencies.
- Install scripts run from the plugin root.
- Install scripts should be idempotent.
- Plugin updates must not delete user-downloaded models unless explicitly
  requested.
- PicAiPic should disable a plugin before uninstalling it.
- Uninstall should remove plugin binaries and environments, but should ask
  before removing large model caches.

## Port Allocation

Local HTTP plugins should avoid fixed-port conflicts.

Preferred model:

1. The manifest declares a default port or `baseUrl`.
2. PicAiPic checks whether the port is free.
3. If the port is busy, PicAiPic may allocate another loopback port.
4. PicAiPic passes the selected port through environment variables.

```json
{
  "entry": {
    "kind": "local-http",
    "baseUrl": "http://127.0.0.1:${PICAIPIC_PLUGIN_PORT}",
    "defaultPort": 8011,
    "startCommand": "start.bat"
  }
}
```

Environment variables passed to the plugin:

```text
PICAIPIC_PLUGIN_ID=picai-nafnet-restore
PICAIPIC_PLUGIN_PORT=8011
PICAIPIC_PLUGIN_ROOT=D:\...\picai-nafnet-restore
PICAIPIC_OUTPUT_DIR=D:\...\PicAiPic Results
```

Plugins should bind to `127.0.0.1` unless the user explicitly opts into a
different bind address.

## Event Namespacing

PicAiPic already uses internal events such as:

```text
index_progress
index_finished
thumbnail_ready
library-total-refreshed
library-folder-sync-finished
dedup-scan-progress
face_index_progress
face_index_finished
```

Plugin-related events must not reuse these names.

Recommended plugin event names:

```text
plugin-task-started
plugin-task-progress
plugin-task-finished
plugin-task-failed
plugin-status-changed
plugin-model-download-progress
```

Event payloads should always include `pluginId`, `capabilityId`, and `taskId`:

```json
{
  "pluginId": "picai-nafnet-restore",
  "capabilityId": "denoise",
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "progress": 42,
  "message": "Processing tile 7 of 16"
}
```

PicAiPic may translate plugin progress into UI state, but plugins should not
emit or depend on album indexing events.

## Capability Kinds

Capability kinds are namespaced and specific. Broad UI grouping can use the
prefix, while dispatch uses the full kind.

```text
image.color.match
image.color.transfer
image.color.lut.export
image.restore.denoise
image.restore.deblur
image.restore.jpeg
image.restore.exposure
image.inpaint.mask
image.segment.semantic
image.segment.subject
image.face.parse
image.beauty.filter
image.background.remove
image.upscale
image.generate.textToImage
image.generate.imageToImage
image.caption
image.tag
image.embedding
utility.device.status
utility.raw.decode
utility.log.convert
```

Plugins may add experimental kinds under `x.<publisher>.<name>`, but stable
PicAiPic UI should prefer the standard kinds.

## Inputs

Version 1 input kinds:

```text
image
mask
lut
text
json
number
boolean
folder
file
```

Image inputs should support file paths by default. A plugin can also request
multipart upload if it cannot read local paths.

Recommended JSON task input:

```json
{
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "capability": "denoise",
  "inputs": {
    "source": {
      "kind": "image",
      "path": "D:\\Photos\\IMG_1001.RW2"
    }
  },
  "parameters": {
    "method": "auto",
    "strength": 0.55
  },
  "outputDir": "D:\\Photos\\PicAiPic Results"
}
```

## Outputs

All plugin invocations should return a normalized result object:

```json
{
  "ok": true,
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "outputs": [
    {
      "id": "result",
      "kind": "image",
      "path": "D:\\Photos\\PicAiPic Results\\IMG_1001-denoise.png",
      "mime": "image/png"
    }
  ],
  "meta": {
    "device": "cuda",
    "elapsedMs": 1840,
    "model": "NAFNet-SIDD-width64"
  }
}
```

Allowed output kinds:

```text
image
mask
lut
json
text
embedding
sidecar
```

For image outputs, prefer real files over base64 strings. Base64 is allowed for
small previews but should not be required for full-resolution results.

## Output Import And Sidecars

PicAiPic should treat plugin results as new files, not edits to originals.

Recommended output naming:

```text
{originalStem}-{capability}-{shortTaskId}.{ext}
IMG_1001-denoise-8ff4f4d2.png
```

Plugins may return sidecar metadata:

```json
{
  "outputs": [
    {
      "id": "result",
      "kind": "image",
      "path": "D:\\Photos\\PicAiPic Results\\IMG_1001-denoise.png",
      "sourceInputId": "source"
    },
    {
      "id": "recipe",
      "kind": "sidecar",
      "path": "D:\\Photos\\PicAiPic Results\\IMG_1001-denoise.json",
      "mime": "application/json"
    }
  ]
}
```

PicAiPic should preserve the relationship between source and result in its
database when the user imports output files.

Suggested imported metadata:

```json
{
  "sourceFileId": 123,
  "pluginId": "picai-nafnet-restore",
  "capability": "denoise",
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "createdAt": "2026-06-18T12:00:00Z"
}
```

### Result Registration Modes

PicAiPic should support three result registration modes.

`copyIntoAlbum`
: Plugin writes to a temporary or plugin output directory. PicAiPic imports the
  result into the selected album folder using the same import flow as user-added
  files. This is the safest default.

`alreadyInAlbum`
: Plugin writes directly to a PicAiPic-provided output directory inside the
  album. PicAiPic validates the path is inside the album and registers it with
  the library database.

`overwriteSource`
: Plugin replaces the original file. This should be disabled by default and
  require explicit user confirmation because it bypasses the non-destructive
  plugin principle.

Recommended task field:

```json
{
  "resultPolicy": "copyIntoAlbum"
}
```

For `copyIntoAlbum`, PicAiPic should call its normal import path and then
refresh folder and library counts. For `alreadyInAlbum`, PicAiPic should add or
refresh the result file in the database. In both cases, thumbnail generation
belongs to PicAiPic.

## Thumbnails And Preview Protocols

PicAiPic owns thumbnail and preview serving. The current app has custom image
schemes for library media:

```text
thumb://localhost/{libraryId}/{fileId}
preview://localhost/{libraryId}/{fileId}
```

Plugins should not generate `thumb://` or `preview://` URLs. They should return
normal output file paths. After PicAiPic imports or registers the output, the
core thumbnail pipeline should generate thumbnails and notify the UI.

Plugins may return an optional temporary preview image for immediate display:

```json
{
  "outputs": [
    {
      "id": "result",
      "kind": "image",
      "path": "D:\\Temp\\plugin-output\\result.png",
      "previewPath": "D:\\Temp\\plugin-output\\result-preview.jpg"
    }
  ]
}
```

`previewPath` is only a temporary preview. The final library preview still comes
from PicAiPic after import.

## Errors

Plugins should return structured errors with stable codes:

```json
{
  "ok": false,
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "error": {
    "code": "model_missing",
    "message": "Required model weights were not found.",
    "details": {
      "path": "models/NAFNet-SIDD-width64.pth"
    }
  }
}
```

Recommended error codes:

```text
plugin_not_ready
invalid_input
unsupported_format
model_missing
dependency_missing
out_of_memory
task_cancelled
task_failed
remote_auth_required
remote_request_failed
```

## Logs And Diagnostics

Each plugin should write logs under its own `logs` directory. PicAiPic should
not scrape arbitrary stdout forever; it should capture startup logs for a
bounded time and then rely on plugin log files or status endpoints.

Recommended diagnostics endpoint:

```text
GET /diagnostics
```

Recommended response:

```json
{
  "pluginId": "picai-nafnet-restore",
  "logFiles": [
    "logs/plugin.log",
    "logs/install.log"
  ],
  "environmentSummary": {
    "runtime": "python",
    "pythonVersion": "3.12.4"
  },
  "lastError": {
    "code": "model_missing",
    "message": "NAFNet-GoPro-width64.pth was not found."
  }
}
```

PicAiPic should expose a "Copy diagnostics" action that includes:

- PicAiPic version.
- OS and architecture.
- Plugin manifest.
- Plugin status response.
- Recent plugin logs.
- Last task error.

Diagnostics should redact API keys, tokens, and user secrets.

## Smoke Tests

Every install profile should be verified by a real plugin smoke test before
PicAiPic marks it usable. A smoke test is stricter than `/status` but cheaper
than a full user task: it should check runtime dependencies, model files,
compute backend availability, and either load the model or run a tiny input.

Recommended endpoint:

```text
POST /smoke-test
```

Recommended request:

```json
{
  "profileId": "windows-amd-rocm",
  "backend": "rocm",
  "capability": "color-transfer",
  "runtimeBindingId": "salut-windows-rocm"
}
```

Recommended response:

```json
{
  "ok": true,
  "passed": true,
  "pluginId": "picai-salut-color",
  "profileId": "windows-amd-rocm",
  "backend": "rocm",
  "capability": "color-transfer",
  "durationMs": 1840,
  "environment": {
    "runtime": "python",
    "pythonVersion": "3.12.4",
    "torch": {
      "available": true,
      "cudaAvailable": true,
      "rocmAvailable": true,
      "hipVersion": "6.3"
    }
  },
  "models": [
    {
      "id": "salut-main",
      "available": true,
      "loaded": true
    }
  ],
  "steps": [
    { "name": "torch", "passed": true },
    { "name": "models", "passed": true },
    { "name": "load-model", "passed": true },
    { "name": "tiny-input", "passed": true }
  ]
}
```

On failure, return `passed: false` with a structured `error` and failed step
names. PicAiPic should update the profile state to `verified` only when the
smoke test passes; diagnostics alone should not mark a profile as usable.
The latest profile smoke state should be persisted by PicAiPic so future
installers, derived runtime profiles, and plugin packaging tools can use the
same `verified` or `failed` signal after restart.

Profile installation state is host-owned. Version 1 uses these states:

```text
notInstalled
installing
needsVerify
verified
failed
```

Clicking Setup may create or update a runtime setup task record, but it must
not mark the profile usable. A completed setup should move to `needsVerify`.
Only a passing smoke test can move a profile to `verified`; a failing smoke
test moves it to `failed`.

PicAiPic should keep setup job records separate from the final profile state.
A setup job is an execution/history record with fields such as `id`,
`pluginId`, `profileId`, `backend`, `status`, `progress`, `message`, `error`,
and `log`. The profile state may point at the latest setup job, but the profile
is still not usable until smoke test verification passes.

## Runtime Bindings

AI plugins should not assume that every install profile owns a private virtual
environment. Large runtimes such as PyTorch, CUDA, ROCm, DirectML, OpenCV, and
diffusers should be reusable when possible.

An install profile may declare a default `runtimeBinding` and optional
`runtimeBindings` candidates:

```json
{
  "id": "windows-amd-rocm",
  "backend": "rocm",
  "label": "AMD ROCm",
  "supportLevel": "derived",
  "runtimeBinding": {
    "scope": "external",
    "kind": "python",
    "id": "salut-windows-rocm",
    "label": "Existing SA-LUT ROCm runtime",
    "python": "D:\\ailab\\20260610133133\\backend\\venv\\Scripts\\python.exe",
    "root": "D:\\ailab\\20260610133133\\backend",
    "requirements": "backend/requirements-rocm.txt"
  },
  "runtimeBindings": [
    {
      "scope": "external",
      "kind": "python",
      "id": "salut-windows-rocm",
      "label": "Existing SA-LUT ROCm runtime",
      "python": "D:\\ailab\\20260610133133\\backend\\venv\\Scripts\\python.exe",
      "root": "D:\\ailab\\20260610133133\\backend",
      "requirements": "backend/requirements-rocm.txt"
    },
    {
      "scope": "plugin",
      "kind": "python",
      "id": "salut-rocm-plugin",
      "label": "Plugin ROCm fallback",
      "requirements": "backend/requirements-rocm.txt"
    }
  ]
}
```

Supported scopes:

```text
external - an existing user or project runtime
shared   - a PicAiPic-managed runtime pool reused by compatible plugins
plugin   - a plugin-private runtime used only when isolation is required
```

`envDir` is only meaningful for plugin-private runtimes. A setup preview should
show the selected runtime scope, Python executable when known, requirements
file, and any warnings. Setup command execution may receive these environment
variables:

```text
PICAIPIC_PLUGIN_RUNTIME_SCOPE
PICAIPIC_PLUGIN_RUNTIME_KIND
PICAIPIC_PLUGIN_RUNTIME_ID
PICAIPIC_PLUGIN_RUNTIME_ROOT
PICAIPIC_PLUGIN_PYTHON
PICAIPIC_PLUGIN_ENV_DIR
PICAIPIC_PLUGIN_ENV_PATH
PICAIPIC_PLUGIN_REQUIREMENTS
PICAIPIC_PLUGIN_REQUIREMENTS_PATH
```

Runtime binding still does not prove usability. PicAiPic must run the profile's
smoke test and mark the profile `verified` only after the smoke test passes.
The selected binding should be passed as `runtimeBindingId` to Setup, setup
preview, Run setup, and Smoke. PicAiPic should persist the selected
`runtimeBinding` snapshot in the profile state so later operations continue to
use the same runtime unless the user changes it.

When PicAiPic discovers an external runtime that was not declared in the
manifest, the request may include a full `runtimeBinding` object as an override
instead of relying only on `runtimeBindingId`. The host should still persist the
selected binding snapshot and require Smoke before marking the profile usable.

## App Data And Cache Paths

Plugins should not hard-code PicAiPic's internal app-data identifier. The
current code resolves app data and cache directories through the Tauri
identifier, and that identifier may change during the PicAiPic rebrand.

PicAiPic should pass paths to plugins explicitly:

```text
PICAIPIC_PLUGIN_ROOT
PICAIPIC_PLUGIN_DATA_DIR
PICAIPIC_PLUGIN_CACHE_DIR
PICAIPIC_TASK_TEMP_DIR
PICAIPIC_OUTPUT_DIR
```

Rules:

- Plugin persistent settings go under `PICAIPIC_PLUGIN_DATA_DIR`.
- Plugin temporary files go under `PICAIPIC_PLUGIN_CACHE_DIR` or
  `PICAIPIC_TASK_TEMP_DIR`.
- Large model files may live under the plugin root, plugin data dir, or a
  user-selected model directory declared in the manifest.
- PicAiPic may clean task temp directories after task completion.
- Plugins should not store state in the photo library database.

## Status And Health

Every plugin should expose health and status.

Minimal health response:

```json
{
  "ready": true,
  "version": "0.1.0"
}
```

Recommended status response:

```json
{
  "ready": true,
  "device": {
    "backend": "cuda",
    "name": "AMD Radeon RX 7900 XTX",
    "memoryMb": 24576
  },
  "models": [
    {
      "id": "nafnet-sidd",
      "available": true,
      "loaded": false,
      "path": "models/NAFNet-SIDD-width64.pth"
    }
  ],
  "capabilities": {
    "denoise": { "available": true },
    "deblur": { "available": false, "reason": "model_missing" }
  }
}
```

## Task Lifecycle

Long-running plugins should use the async task contract. `POST /invoke/{capability}`
should return quickly with `202 Accepted`, a `taskId`, the initial status, and
tracking endpoints. The actual work should run in a plugin-managed worker queue.

Version 1 task endpoints:

```text
POST /invoke/{capability}
GET  /tasks/{taskId}
GET  /tasks/{taskId}/events?after={seq}&timeoutMs={ms}
POST /tasks/{taskId}/cancel
```

Task status values:

```text
queued
running
cancelling
succeeded
failed
cancelled
```

`completed` may be accepted as an alias from plugins, but PicAiPic normalizes a
successful terminal task to `succeeded` in its host ledger.

Async invoke response:

```json
{
  "ok": true,
  "async": true,
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "status": "queued",
  "events": {
    "method": "GET",
    "path": "/tasks/8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81/events",
    "cursor": 0,
    "timeoutMs": 25000
  },
  "poll": {
    "method": "GET",
    "path": "/tasks/8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
    "intervalMs": 1000
  }
}
```

Task status response:

```json
{
  "ok": true,
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "status": "running",
  "progress": 42,
  "message": "Processing tile 7 of 16"
}
```

Long-poll task events response:

```json
{
  "ok": true,
  "taskId": "8ff4f4d2-68c2-4bd6-b4f8-b8ac8ab6df81",
  "status": "running",
  "terminal": false,
  "nextCursor": 12,
  "events": [
    {
      "seq": 12,
      "type": "task.progress",
      "status": "running",
      "progress": 42,
      "message": "Processing tile 7 of 16",
      "at": "2026-06-21T04:52:26Z"
    }
  ],
  "state": {
    "status": "running",
    "progress": 42,
    "message": "Processing tile 7 of 16"
  }
}
```

`GET /tasks/{taskId}/events` should hold the request until a later event is
available, the task reaches a terminal state, or `timeoutMs` expires. It should
return an empty `events` array on timeout. Hosts should resume with `after` set
to the previous `nextCursor`. If a plugin does not implement events, the host may
fall back to `GET /tasks/{taskId}` with conservative polling.

Cancellation is cooperative. A queued task may transition directly to
`cancelled`; a running task should transition to `cancelling` and then
`cancelled` at the next safe checkpoint. Plugins must not claim hard interruption
for a blocking model call unless they truly support it.

Simple plugins may still return a final result directly from `POST /invoke/...`,
but long-running AI plugins should prefer the async task contract.

PicAiPic keeps a host-owned task ledger for invoked plugin work. Successful
outputs are retained until import/adopt/discard. Failed and cancelled task
directories are cleaned best-effort. Unadopted successful outputs may be
expired by host TTL cleanup and marked `discarded` in the ledger, so plugins
should treat returned task output paths as handoff files rather than permanent
storage.

## Concurrency And Resource Control

AI plugins can compete for VRAM, RAM, CPU, and disk IO. PicAiPic should include a
basic resource coordinator.

Manifest resource hints:

```json
{
  "resources": {
    "maxConcurrentTasks": 1,
    "estimatedVramMb": 8192,
    "estimatedRamMb": 4096,
    "exclusiveGpu": true
  }
}
```

Rules:

- Default to one active task per plugin.
- Do not run two `exclusiveGpu` tasks at the same time on the same GPU.
- Let CPU-only lightweight tasks run concurrently when safe.
- Queue tasks rather than launching unbounded processes.
- Cancellation should be best-effort and should clean temporary files.
- PicAiPic should display queued, running, cancelled, failed, and completed
  states.

Plugins may reject a task with `out_of_memory` or `busy`. PicAiPic should keep
the task in a recoverable state so the user can retry with CPU or smaller
settings.

## Parameter Schema

Capability parameters use a JSON Schema subset:

```text
type
properties
required
default
enum
minimum
maximum
description
ui
```

The optional `ui` field gives PicAiPic hints without hard-coding plugin UI:

```json
{
  "strength": {
    "type": "number",
    "minimum": 0,
    "maximum": 1,
    "default": 0.55,
    "ui": {
      "control": "slider",
      "step": 0.01
    }
  }
}
```

## Compatibility Rules

PicAiPic should be liberal in what it accepts and strict in what it sends.

Plugins should:

- Accept absolute Windows paths.
- Accept paths containing spaces and non-ASCII characters.
- Preserve EXIF orientation when possible or report when it is discarded.
- Support PNG and JPEG at minimum for image output.
- Avoid fixed ports unless declared in the manifest.
- Use unique default ports to avoid collisions.
- Keep logs under the plugin directory.
- Use deterministic output file names when PicAiPic provides `outputDir`.

PicAiPic should:

- Validate the manifest before enabling a plugin.
- Prefer file path transfer for local plugins.
- Fall back to multipart upload only when requested.
- Create `outputDir` before invocation.
- Import outputs only after the plugin reports success.
- Never assume a plugin supports GPU just because a model usually can.

## Environment And Device Compatibility

Compatibility detection is split between PicAiPic and each plugin.

PicAiPic owns platform-level detection and user-facing coordination. A plugin
owns dependency-level detection and real runtime validation.

### PicAiPic Responsibilities

PicAiPic should detect and display:

- Operating system and architecture, such as `windows-x64`.
- CPU architecture and available system memory.
- Available disk space for plugin installation and output files.
- GPU adapters visible to Windows.
- Basic GPU vendor classification: NVIDIA, AMD, Intel, Microsoft Basic Render,
  or unknown.
- Approximate dedicated GPU memory when available.
- Plugin manifest validity.
- Whether a plugin declares support for the current platform and hardware.

PicAiPic should not try to understand every plugin's internal dependency stack.
It should not assume that PyTorch, ONNX Runtime, CUDA, ROCm, DirectML, OpenVINO,
or model weights are actually usable until the plugin reports that status.

### Plugin Responsibilities

Each plugin should detect and report:

- Whether its runtime is installed, such as Python, Node, a native executable,
  or an embedded runtime.
- Whether required packages are installed, such as `torch`, `onnxruntime`,
  `onnxruntime-directml`, `opencv-python`, or `diffusers`.
- Whether model files exist and, when known, match expected hashes.
- Which compute backends are actually usable after a small runtime probe.
- Which backend is selected for each capability.
- Whether a capability can run now, can run after model download, or is blocked.
- The reason for fallback when a preferred backend fails.

Runtime probing should be real, not only package-name based. For example, a
PyTorch CUDA plugin should allocate a tiny tensor and execute a trivial
operation before reporting CUDA as usable.

### Manifest Hardware Declaration

The manifest declares what the plugin is designed to support. It does not prove
that the user's machine is ready.

```json
{
  "hardware": {
    "cpu": true,
    "cuda": true,
    "rocm": true,
    "directml": true,
    "openvino": false,
    "minVramMb": 4096,
    "recommendedVramMb": 8192
  }
}
```

Field meaning:

`cpu`
: Plugin can run on CPU.

`cuda`
: Plugin can use NVIDIA CUDA or a PyTorch ROCm build that exposes devices
  through the CUDA API.

`rocm`
: Plugin explicitly supports AMD ROCm. On Windows this should be declared only
  when the plugin has been tested with a ROCm Windows stack.

`directml`
: Plugin supports DirectML, usually through ONNX Runtime DirectML or
  `torch-directml`.

`openvino`
: Plugin supports OpenVINO.

`minVramMb`
: Minimum recommended GPU memory for GPU execution.

`recommendedVramMb`
: Memory target for good-quality defaults.

### Device Preference In Tasks

PicAiPic sends a device preference with each task. The plugin makes the final
choice.

```json
{
  "runtime": {
    "preferredDevice": "auto",
    "fallback": ["cuda", "rocm", "directml", "openvino", "cpu"]
  }
}
```

`preferredDevice` values:

```text
auto
cpu
cuda
rocm
directml
openvino
```

If `preferredDevice` is `auto`, the plugin may choose its best available device.
If a specific backend is requested but fails, the plugin may follow the fallback
list unless the user disabled fallback in PicAiPic settings.

### Recommended Auto Priority

PicAiPic may generate a default fallback list using platform and GPU vendor:

| Platform / GPU | Recommended order |
| --- | --- |
| Windows + NVIDIA | `cuda`, `directml`, `cpu` |
| Windows + AMD | `directml`, `rocm`, `cpu` |
| Windows + Intel | `directml`, `openvino`, `cpu` |
| Linux + NVIDIA | `cuda`, `cpu` |
| Linux + AMD | `rocm`, `cpu` |
| macOS Apple Silicon | plugin-specific, usually `mps`, then `cpu` |
| Unknown / no GPU | `cpu` |

Plugins may override this when their runtime has better knowledge. For example,
an ONNX plugin may prefer `DmlExecutionProvider` on Windows AMD, while a PyTorch
plugin may prefer CPU if DirectML is not implemented for the needed operators.

### CUDA And ROCm Naming

Some PyTorch ROCm builds expose AMD GPU execution through APIs named `cuda`.
Plugins should report both the API name and the real backend when possible:

```json
{
  "id": "cuda:0",
  "api": "cuda",
  "backend": "rocm",
  "vendor": "amd",
  "name": "AMD Radeon RX 7900 XTX",
  "available": true
}
```

For NVIDIA CUDA, report:

```json
{
  "id": "cuda:0",
  "api": "cuda",
  "backend": "cuda",
  "vendor": "nvidia",
  "name": "NVIDIA GeForce RTX 4090",
  "available": true
}
```

This distinction lets PicAiPic show a correct user-facing label while allowing
plugins to use the framework API they actually need.

### Status Response

A plugin status endpoint should include environment, devices, models, and
capability availability.

```json
{
  "ready": true,
  "plugin": {
    "id": "picai-nafnet-restore",
    "version": "0.1.0"
  },
  "environment": {
    "runtime": "python",
    "pythonVersion": "3.12.4",
    "packages": [
      {
        "name": "torch",
        "version": "2.7.1+rocm",
        "available": true
      }
    ]
  },
  "devices": [
    {
      "id": "cuda:0",
      "api": "cuda",
      "backend": "rocm",
      "vendor": "amd",
      "name": "AMD Radeon RX 7900 XTX",
      "memoryMb": 24576,
      "available": true,
      "recommended": true,
      "probe": {
        "ok": true,
        "elapsedMs": 19
      }
    },
    {
      "id": "cpu",
      "api": "cpu",
      "backend": "cpu",
      "available": true
    }
  ],
  "selectedDevice": "cuda:0",
  "models": [
    {
      "id": "nafnet-sidd",
      "available": true,
      "loaded": false,
      "path": "models/NAFNet-SIDD-width64.pth"
    }
  ],
  "capabilities": {
    "denoise": {
      "available": true,
      "devices": ["cuda:0", "cpu"]
    },
    "deblur": {
      "available": false,
      "reason": "model_missing",
      "message": "NAFNet-GoPro-width64.pth was not found."
    }
  }
}
```

### Failed Compatibility Response

If a plugin starts but cannot run a capability, it should still return a useful
status:

```json
{
  "ready": false,
  "environment": {
    "runtime": "python",
    "pythonVersion": "3.12.4"
  },
  "devices": [
    {
      "id": "cuda:0",
      "api": "cuda",
      "backend": "cuda",
      "available": false,
      "reason": "torch_cuda_unavailable",
      "message": "PyTorch was installed without CUDA support."
    },
    {
      "id": "cpu",
      "api": "cpu",
      "backend": "cpu",
      "available": true
    }
  ],
  "capabilities": {
    "denoise": {
      "available": false,
      "reason": "dependency_missing",
      "message": "opencv-python is not installed."
    }
  }
}
```

### Invocation Result Device Metadata

Every successful task should report the actual backend used:

```json
{
  "ok": true,
  "outputs": [
    {
      "id": "result",
      "kind": "image",
      "path": "D:\\Photos\\PicAiPic Results\\IMG_1001-denoise.png"
    }
  ],
  "meta": {
    "requestedDevice": "auto",
    "selectedDevice": "cuda:0",
    "backend": "rocm",
    "fallbackUsed": false,
    "elapsedMs": 1840
  }
}
```

If fallback was used:

```json
{
  "meta": {
    "requestedDevice": "cuda",
    "selectedDevice": "cpu",
    "backend": "cpu",
    "fallbackUsed": true,
    "fallbackReason": "cuda_out_of_memory"
  }
}
```

### Detection Timing

PicAiPic should run only cheap platform detection at startup. Plugin checks can
be staged:

1. Manifest validation when the plugin is discovered.
2. Cheap status check when the plugin list is opened.
3. Full dependency and device probe when the user enables the plugin or opens an
   AI tool that needs it.
4. Model load only when a task actually needs that model, unless the plugin
   explicitly supports preloading.

This keeps PicAiPic startup fast and avoids loading large AI stacks unless the
user asks for them.

## Security And Network Policy

Version 1 assumes local plugins. If a plugin calls a remote API, it must declare
that in the manifest:

```json
{
  "network": {
    "required": true,
    "hosts": ["api.openai.com"],
    "userProvidesCredentials": true
  }
}
```

PicAiPic should show this before enabling the plugin. Local plugins should bind
to `127.0.0.1`, not `0.0.0.0`, unless the user explicitly changes it.

## Model Acquisition

Plugins should declare model files, but PicAiPic should not assume it can
download them automatically.

Model declaration:

```json
{
  "models": [
    {
      "id": "nafnet-sidd",
      "name": "NAFNet SIDD",
      "required": false,
      "path": "models/NAFNet-SIDD-width64.pth",
      "sha256": null,
      "sizeMb": 270,
      "license": "Apache-2.0",
      "download": {
        "kind": "manual",
        "url": "https://github.com/megvii-research/NAFNet"
      }
    }
  ]
}
```

Download kinds:

```text
manual
direct
script
huggingface
github-release
```

Rules:

- PicAiPic should show model license and size before download.
- Plugins should validate model files when hashes are available.
- Model download failures should not disable the whole plugin if other
  capabilities still work.
- Plugins should support external model directories when users already have
  weights installed.

## File Format Policy

PicAiPic and plugins should agree on minimum file format behavior.

Inputs:

- PicAiPic may pass original image paths, including RAW files.
- A plugin must report `unsupported_format` if it cannot read an input.
- If a plugin decodes RAW itself, it should report the decode method in `meta`.
- If PicAiPic pre-renders a temporary image for compatibility, it should pass
  both `path` and `originalPath` when possible.

Outputs:

- PNG is the default lossless output.
- JPEG output should report quality when applicable.
- TIFF output should report bit depth when applicable.
- LUT outputs should use `.cube` unless a capability declares another format.
- Color profile handling should be reported in `meta` when known.

Example output metadata:

```json
{
  "meta": {
    "outputFormat": "png",
    "colorProfile": "sRGB",
    "exifPreserved": false,
    "rawDecode": "plugin-rawpy-prophoto-linear"
  }
}
```

## Source-Based Implementation Order

Based on the current codebase, the lowest-risk implementation sequence is:

1. Add plugin manifest parsing and registry storage beside the existing app
   config.
2. Add Tauri commands for listing plugins and reading plugin status.
3. Add a plugin task runner that invokes `local-http` or `local-command`
   without exposing Tauri APIs to plugin UI.
4. Store plugin outputs in an app-managed temp or output directory.
5. Reuse the existing import/register flow to add successful outputs to the
   current album.
6. Let the existing thumbnail pipeline create thumbnails and emit UI refresh
   events.
7. Add settings UI for enable/disable, diagnostics, device preference, and model
   status.
8. Only then add richer tool panels for individual plugin capabilities.

This sequence keeps the existing browsing, editing, indexing, and thumbnail code
in charge of the library while AI plugins remain replaceable external workers.

## Upstream Project Wrapping

Each GitHub or local upstream project should be wrapped as its own PicAiPic
plugin. Prototype code can be used as a reference, but PicAiPic should not ship
one large plugin that mixes unrelated upstream projects.

Recommended mapping:

| Upstream project | Plugin package | Capability kinds |
| --- | --- | --- |
| `D:\ailab\SA-LUT-main` | `picai-salut-color` | `image.color.transfer`, `image.color.lut.export` |
| `D:\ailab\NAFNet` | `picai-nafnet-restore` | `image.restore.denoise`, `image.restore.deblur`, `image.restore.jpeg` |
| `D:\ailab\IOPaint-main` | `picai-iopaint-inpaint` | `image.inpaint.mask` |
| MobileSAM source | `picai-mobile-sam-segment` | `image.segment.semantic`, `image.segment.subject` |
| Illumination-Adaptive Transformer source | `picai-iat-exposure` | `image.restore.exposure` |
| GPUPixel source | `picai-gpupixel-filter` | `image.beauty.filter` |
| OpenAI-compatible vision color recipe adapter | `picai-banana-color` | `image.color.transfer`, `image.color.lut.export` |

Rules for wrapping an upstream project:

- Keep upstream source and model dependencies inside the plugin package or a
  user-configured external model/source path.
- Put PicAiPic-specific code in a thin adapter layer.
- Expose only PicAiPic's standard `/health`, `/status`, and `/invoke/...`
  contract to the host app.
- Do not make PicAiPic import upstream Python modules directly.
- Do not make one plugin depend on another plugin unless a future manifest field
  explicitly declares that relationship.

For example, `picai-salut-color` can wrap `D:\ailab\SA-LUT-main`, but PicAiPic
should only see:

```text
GET  /health
GET  /status
POST /invoke/color-transfer
POST /invoke/export-lut
```

The internal details of SA-LUT remain inside that plugin.

## Version 1 Implementation Plan

1. Add manifest validation in PicAiPic.
2. Add a registered plugin paths list.
3. Discover installed manifests.
4. Show plugin status and capabilities in settings.
5. Start `local-http` plugins on demand.
6. Invoke capabilities through normalized task JSON.
7. Save outputs to a PicAiPic results folder.
8. Let the user import output files into the current album.
9. Add cancellation and progress polling for long tasks.

This gives PicAiPic a stable plugin boundary while keeping AI projects
independent, replaceable, and easier to update.
