# starbott (Rust CLI)

Terminal client for Starbott backend APIs.

## Build

```bash
cd cli
cargo build --release
```

Binary output:

```bash
./target/release/starbott
```

## Config

Config file path:

- macOS/Linux: `~/.config/starbott/config.json`
- Windows: `%APPDATA%\\starbott\\config.json`

Initialize:

```bash
starbott config init --api-url http://localhost:3003
```

Set token:

```bash
starbott auth login --token "<jwt>"
```

## Commands

- `starbott config init|get|set|profiles|use`
- `starbott auth login|logout`
- `starbott workspaces create|list|permissions`
- `starbott tools propose|commit|deny|runs`
- `starbott whoami`
- `starbott chat "<prompt>" [--stdin] [-m|--model <selector>] [--stream]`
- `starbott tui [-m|--model <selector>]`
- `starbott usage [--since <value>] [--until <value>] [--group day|model|provider]`
- `starbott billing status`
- `starbott billing portal [--open]`
- `starbott health`

## TUI

Fullscreen chat UI for quick testing.

```bash
starbott tui
starbott tui -m vertex:gemini-3-flash-preview
```

Cute mode toggle (SPEC5):

Create `~/.starbott/config` with:

```ini
cute = on        # phrases + icons + spinner
# cute = minimal # icons/spinner only
# cute = off     # plain status text (no cute phrases)
```

Keys:

- `Enter` send
- `F2` or `Ctrl+M` model picker
- `PgUp/PgDn` scroll
- `Ctrl+R` reload `/v1/models`
- `Ctrl+D` toggle debug panel
- `Esc` quit

Convenience (dev) wrapper that logs in and runs the CLI:

```bash
./scripts/starbott-dev.sh tui
```

## Global flags

- `--profile <name>`
- `--api-url <url>`
- `--json`
- `--quiet`
- `--timeout <ms>` (default `30000`)
- `--retries <n>` (default `2`)
- `--verbose`
- `--debug`

## Exit codes

- `0` success
- `1` generic error
- `2` auth error
- `3` usage error
- `4` network error / timeout
- `5` rate limited
- `6` server error
