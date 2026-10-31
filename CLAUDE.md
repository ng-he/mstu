# Working on mstu

A media pipeline. Plugins are shared libraries the engine loads and runs as
nodes of a pipeline; the desktop app draws the pipeline and hosts each
plugin's own page.

This repository holds no code. Every part is its own repository, brought in
as a submodule, so a plugin can be written in any language and released on
its own schedule.

| | |
| --- | --- |
| `mstu-sdk` | The ABI plugins are written against, plus the generated C header. |
| `mstu-media` | Codecs, four-character codes, NAL units. Used by plugins, not by the engine. |
| `mstu-engine` | Loads plugins, runs the pipeline, serves the desktop over a unix socket. |
| `mstu-desktop` | Electron and React, and the UI SDK a plugin page is written against. |
| `plugins-src/*` | One repository per plugin. |
| `integration/` | The parts tested together against a running engine. |
| `plugins/` | Where built plugins are collected. Gitignored; the engine reads it. |

## After every change

```sh
./fmt.sh             # format every part written in Rust
./test.sh            # every part, then the parts together
./test.sh --unit     # only the parts, when nothing needs the engine
```

They need nothing installed beyond cargo, with one exception:
`mstu-image-decoder` builds against libav, and its own `test.sh` says so and
passes when libav is missing, so a fresh clone stays green. The windows check
skips any part carrying a `.no-windows-check` file, for the same reason. Run
them before saying a change works. **Formatting is not optional**: `./test.sh` runs `./fmt.sh --check`
first and counts unformatted code as a failure, and so does each plugin's own
`test.sh`. Default rustfmt, no `rustfmt.toml` anywhere — don't hand-format
around it.

A change to a plugin only reaches the engine once it is rebuilt and collected,
which `./test.sh` does, or `./collect.sh --build` on its own. **A stale `.so`
in `plugins/` is a segfault waiting to happen**: the engine loads whatever is
there, and an old binary against a changed ABI crashes rather than complains.

## Running it

```sh
cargo run --release --manifest-path mstu-engine/Cargo.toml -- --serve
npm --prefix mstu-desktop run dev
```

`./build.sh` puts the engine and the collected plugins in `build/`, with a
`serve.sh` beside them, and `--with-app` packages the desktop app in as well
through electron builder. `build/` is gitignored.

The engine does nothing without `--serve`. It reads plugins from
`MSTU_PLUGIN_DIR`, or the nearest `plugins` folder from the working directory
upwards. It listens on `MSTU_SOCKET`, or `$XDG_RUNTIME_DIR/mstu.sock`, or
`\\.\pipe\mstu` on Windows; a second engine runs beside a real one by being
given a name of its own, which is how the integration tests stay out of the way.

## More than one platform

Nothing may assume Linux. `mstu-engine/src/ipc.rs` is where the unix socket and
the Windows named pipe part company, with the same split on the client side in
`integration/src/channel.rs`; above those two files the protocol is the same.
A library is `std::env::consts::DLL_EXTENSION` and may or may not carry `lib` in
front of it, so never spell `.so` out. Scripts are bash, run under Git Bash on
Windows.

Nothing here is run on Windows, so the check that catches a mistake is:

```sh
cargo check --target x86_64-pc-windows-msvc --all-targets --manifest-path <part>/Cargo.toml
```

## How the parts fit

The engine owns every byte a message carries. A plugin either writes into
memory the engine reserved for it, or hands over values the engine copies;
it allocates nothing per frame. One payload is shared by every consumer of a
node rather than copied, and lives as long as the last of them.

A plugin exports one symbol, `plugin_descriptor`, returning a table of
function pointers. Each plugin repository provides `build.sh` and `test.sh`;
the centre calls those and never assumes a language. Built libraries and `ui/`
folders are gathered by `collect.sh` into `plugins/`.

Inside this checkout `.cargo/config.toml` patches `mstu-sdk` and `mstu-media`
to the submodules, so a change to the SDK reaches everything without
publishing. Neither crate is on crates.io yet.

## Conventions

- Comments are one line. Reasoning goes in the conversation, not the source.
- Commit messages are a subject and a line or two, for what the diff cannot
  say: a bug's cause, a constraint, something left out on purpose.
- Submodules track a commit: change a part, commit and push it there, then
  record the move here.
- Don't commit unless asked.

## Things that have bitten

- **Tests sharing process state.** The engine has one event tap, one live
  board, one allocator counter. A test touching any of them takes
  `runtime::fake::globals()` first, or it races the test beside it.
- **Wall-clock assertions.** A test that times two loops and compares them
  fails about once in thirty on a loaded machine. Assert what it means —
  pointers, counts, bytes — and keep the timing as an `#[ignore]`d
  measurement.
- **A test that has never failed proves nothing.** Break it on purpose once
  and watch it go red; several tests here were vacuous until that was done.
- **`str.replace` in an edit script fails silently** when the pattern does not
  match, leaving the test green and the change absent. Assert the match first.
- **`mstu-sdk/abi/mstu-sdk.h` is regenerated by `build.rs`** on every build.
  Editing it by hand does nothing.
