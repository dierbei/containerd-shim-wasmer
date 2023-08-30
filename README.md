# Build
```shell
cargo build --release
```

# Move to Containerd Path
```shell
cp target/release/containerd-shim-wasmer-v1 /usr/local/bin
```

# Test
```shell
ctr run --rm --runtime=io.containerd.wasmer.v1 ghcr.io/containerd/runwasi/wasi-demo-app:latest testwasm
```

# Ref
- https://github.com/containerd/runwasi

# Unit Test
```shell
export RUST_LOG=trace
cargo test -- --show-output
```
