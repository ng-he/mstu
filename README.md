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
| [mstu-desktop](https://github.com/ng-he/mstu-desktop) | The Electron and React app. |
| `plugins-src/*` | One repository per plugin. |

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

## Working across repositories

Submodules track a commit, not a branch, so a change to a part is committed and
pushed in that part's repository first, and this repository then records where
it has moved to:

```sh
git -C mstu-engine commit -am "..." && git -C mstu-engine push
git commit -am "chore: move mstu-engine forward"
```

`git submodule update --remote` brings every part up to its latest.
