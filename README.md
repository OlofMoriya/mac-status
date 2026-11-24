# Mac Status

Read-only macOS setup and backup status viewer. Supply a TOML plan with `mac-status --config PLAN.toml`. Add `--json` for machine-readable output or `--gui` for the live native dashboard. The GUI uses Apple's Swift toolchain and refreshes every five seconds. The CLI does not install, fetch, or back up anything.

Install: `brew install olofmoriya/tap/mac-status`. Register the searchable app with `mac-status --config PLAN.toml --install-launcher`. This creates `~/Applications/Mac Status.app` and points it at the installed binary and your chosen plan.

```toml
[[checks]]
kind = "tool"
name = "git"
version = true
```
