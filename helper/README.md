# Helper Folder

This folder is for the Rust `adr-optimizer` binaries used by the future Anki
add-on.

The GitHub Actions workflow builds four platform artifacts. For add-on
packaging, download the assembled `adr-optimizer-helper` artifact from GitHub
Actions, extract `adr-optimizer-helper.tar.gz`, and copy these four folders into
the add-on's `helper` folder:

```text
helper/adr-optimizer-windows-x86_64/adr-optimizer.exe
helper/adr-optimizer-linux-x86_64/adr-optimizer
helper/adr-optimizer-macos-aarch64/adr-optimizer
helper/adr-optimizer-macos-x86_64/adr-optimizer
```

If using the individual workflow artifacts instead, extract each archive into a
folder with the same artifact name before copying it into `helper`.

The loose `helper/adr-optimizer.exe` file is only for local Windows build
testing. It is ignored by git and should not be treated as the complete packaged
helper set.

The Python bridge checks the current platform-specific folder automatically. It
also accepts `ADR_OPTIMIZER_BINARY` when the add-on or a local test needs to
point at a binary somewhere else.
