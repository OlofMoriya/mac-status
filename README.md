# Mac Status

Read-only macOS setup and backup status viewer. Supply a TOML plan with `mac-status --config PLAN.toml`. Add `--json` for machine-readable output or `--gui` for the live native dashboard. The GUI uses Apple's Swift toolchain and refreshes every five seconds. The CLI does not install, fetch, or back up anything.

Install: `brew install olofmoriya/tap/mac-status`.

```toml
[[checks]]
kind = "tool"
name = "git"
version = true
```
