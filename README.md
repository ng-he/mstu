# mstu

A media pipeline: plugins are shared libraries, the engine runs them as nodes
and routes messages between them, and the desktop app is where pipelines are
drawn and wired.

This repository holds no code of its own. Every part is its own repository,
brought in here as a submodule, so a plugin can be written in any language
and released on its own schedule.

```sh
git clone --recurse-submodules https://github.com/ng-he/mstu.git
```

| Submodule | What it is |
| --- | --- |
| [mstu-sdk](https://github.com/ng-he/mstu-sdk) | The ABI plugins are written against. Published to crates.io, with a C header for everyone else. |
| [mstu-media](https://github.com/ng-he/mstu-media) | Media helpers plugins share: codecs, four-character codes, NAL units. Published to crates.io. |
| [mstu-engine](https://github.com/ng-he/mstu-engine) | Loads plugins, runs the pipeline, serves the desktop over a unix socket. |
| [mstu-desktop](https://github.com/ng-he/mstu-desktop) | The Electron and React app, and the UI SDK a plugin's own page is written against. |
| `plugins-src/*` | One repository per plugin. |
| `integration/` | The parts tested together, against a running engine. |

## Building

Each plugin is built by its own repository, in its own language. `collect.sh`
gathers the results into `plugins/`, which is where the engine looks:

```sh
./collect.sh --build          # build every plugin, then collect
./collect.sh                  # collect what is already built
```

A plugin in a language cargo knows nothing about puts its library and its `ui`
folder in `dist/`, and is collected the same way.

Then run the engine and the app:

```sh
cargo run --release --manifest-path mstu-engine/Cargo.toml -- --serve
npm --prefix mstu-desktop run dev
```

Inside this checkout, `.cargo/config.toml` points `mstu-sdk` and `mstu-media` at
the submodules, so a change to the SDK is picked up by the engine and the
plugins without publishing anything. A repository cloned on its own builds
against the published crates instead.

### Linux, macOS, Windows

The engine serves a unix socket where there are unix sockets and a named pipe on
Windows, and the desktop app connects to whichever its platform has; `MSTU_SOCKET`
names another one, for a second engine beside a real one. Plugin libraries are
`.so`, `.dylib` or `.dll` as the platform has it, with or without the `lib` in
front, and are collected and loaded by that name.

The scripts here are bash. Windows runs them under the Git Bash that comes with
Git for Windows; a plugin written for Windows alone may ship a `build.ps1` of its
own, as long as `build.sh` and `test.sh` are what the centre can call.

## Testing

```sh
./test.sh           # every part on its own, then the parts together
./test.sh --unit    # only the parts, needing neither an engine nor media
./fmt.sh            # format every part written in Rust
```

Unformatted code fails the suite, so `./fmt.sh` is the first thing to reach for
if `./test.sh` complains about formatting. It is plain rustfmt with no
configuration; a plugin in another language formats itself, with a `fmt.sh` of
its own if it wants the centre to call it.

The parts written here are tested with cargo. A plugin is tested by its own
`test.sh`, the same way it is built by its own `build.sh`, so a plugin written
in another language tests itself however suits it; all the suite asks of it is
an exit status.

The ones in [integration/](integration) start a real engine, wire real plugins
through it and check what comes out the other end; they need the engine built
and the plugins collected, which `./test.sh` does first.

Media comes from `plugins-src/mstu-media-file-source/test_samples`, a handful of
short clips committed beside the plugin that reads them, so nothing has to be
encoded to run the tests. The script that made them is in that folder.

## Working across repositories

Submodules track a commit, not a branch, so a change to a part is committed and
pushed in that part's repository first, and this repository then records where
it has moved to:

```sh
git -C mstu-engine commit -am "..." && git -C mstu-engine push
git commit -am "chore: move mstu-engine forward"
```

`git submodule update --remote` brings every part up to its latest.
