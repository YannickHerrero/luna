# Luna TUI

`luna-tui` is Luna's disposable Rust terminal client. It uses the same authenticated HTTP and retained WebSocket protocol as the web and Apple clients; it never reads SQLite, Pi JSONL, or raw Pi process output.

## Build and install

From the repository root:

```sh
pnpm build:tui
install -d "$HOME/.local/bin"
install -m 755 target/release/luna-tui "$HOME/.local/bin/luna-tui"
```

Ensure `~/.local/bin` is available to non-interactive SSH commands, or invoke the binary by its absolute path.

## Pair once

The Luna host must be able to reach its private Tailscale HTTPS origin. On first launch:

```sh
luna-tui --server https://your-mac.example.ts.net:8447
```

The client requests a fresh six-digit pairing code. Read the newest code from the Luna/Citadel server log, enter it at the prompt, and then the terminal interface opens. HTTP is accepted only for `localhost` and loopback development servers.

Pairing profiles live outside the repository at:

```text
~/.config/luna/tui/<profile>.json
```

The directory is mode `700` and profiles are mode `600`. A profile contains a bearer credential: do not print, copy into shell history, or commit it. To use a separate logical device identity:

```sh
luna-tui --profile laptop --server https://your-mac.example.ts.net:8447
```

To replace a revoked or invalid local profile after obtaining a new code:

```sh
luna-tui --profile laptop --server https://your-mac.example.ts.net:8447 pair --replace
```

## Use through SSH

After pairing:

```sh
ssh -t luna-host luna-tui
```

or, if the install directory is not in the remote command path:

```sh
ssh -t luna-host '$HOME/.local/bin/luna-tui'
```

Every invocation is a new UI process. Quitting closes only its WebSocket and leaves Pi work running. Reopening fetches durable state and catches up through retained events. Multiple processes can use the same profile simultaneously; named profiles are available when separate device attribution is preferred.

## Appearance

The TUI leaves the terminal's default foreground and background untouched. Focus, status, warning, and error accents use standard ANSI colors, so their actual values come from the active terminal palette. Set `NO_COLOR=1` to disable color accents while retaining text emphasis and selection.

## Keys

| Key              | Action                                            |
| ---------------- | ------------------------------------------------- |
| `Tab`            | Cycle conversation list, transcript, and composer |
| `Ctrl-H/J/K/L`   | Move focus left, down, up, or right               |
| `↑`/`↓`, `j`/`k` | Navigate or scroll                                |
| `Enter`          | Open a conversation or send a message             |
| `Alt-Enter`      | Insert a composer newline                         |
| `i`              | Focus the composer                                |
| `n`              | Create a conversation                             |
| `s`              | Confirm interruption of active work               |
| `PageUp`         | Load earlier messages                             |
| `End`            | Return to live output                             |
| `?`              | Toggle help                                       |
| `q`, `Ctrl-C`    | Quit without interrupting Pi                      |

The conversation list has focus on launch. `Ctrl-H/J/K/L` uses the terminal keyboard-enhancement protocol to distinguish `Ctrl-H` from Backspace and `Ctrl-J` from Enter; `Tab` remains the fallback in terminals that do not support enhanced keyboard events.

Bracketed multiline paste is supported. `!` messages use Luna's existing bounded shell-command path. Resize events are handled automatically, with a single-pane fallback for narrow terminals.

## MVP limitations

The initial client intentionally omits attachment upload and image display, voice transcription, persisted drafts, archived-conversation management, full Markdown/syntax highlighting, model selection, context compaction, and notifications.
