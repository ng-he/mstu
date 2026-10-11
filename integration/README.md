# integration

The plugins and the engine, tested together.

Everything here runs for real: the engine is started as its own process with
its own socket, the plugins are the libraries in `../plugins`, and the media is
the sample files committed beside the source plugin. Nothing is mocked, so a
test failing here means the pipeline is broken, not the test.

```sh
cargo build --release --manifest-path ../mstu-engine/Cargo.toml
../collect.sh --build
cargo test
```

`../test.sh` does all three, along with every submodule's own tests.

A test that cannot run says so rather than passing quietly: without the source
plugin checked out the ones needing media print `skipped: ...`, and without a
built engine or collected plugins the suite fails with the command to run.

Where ffprobe is installed the dumps are decoded as a second opinion; where it
is not, that part is skipped and the rest still runs.

Each test gets its own engine, its own runtime directory and its own output
directory, so they can run at the same time and leave nothing behind.
