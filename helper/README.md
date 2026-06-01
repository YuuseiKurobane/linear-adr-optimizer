# Helper Folder

Put the release Rust executable here when packaging for Anki or running without a
local Cargo build.

Expected names:

- Windows: `helper/adr-optimizer.exe`
- macOS/Linux: `helper/adr-optimizer`

Platform artifact folders are also supported, for example:

```text
helper/adr-optimizer-windows-x86_64/adr-optimizer.exe
helper/adr-optimizer-macos-aarch64/adr-optimizer
helper/adr-optimizer-linux-x86_64/adr-optimizer
```

The Python bridge also accepts `ADR_OPTIMIZER_BINARY` if the add-on wants to
point at a binary outside this folder.
