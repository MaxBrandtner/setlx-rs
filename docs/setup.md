# Compile

```bash
cargo build
```

# Testing

```bash
crago test -- --nocapture # view debug info
cargo test                # only view debug info on failure
```

# Debug

```bash
cargo run -- -fdump-all input.stlx        # debug build
dot -Tpng input-cst-tree.dot -o input.png
```

# Install

```
cargo install --path=.
```

# Run

```bash
setlx-rs input.stlx     # installed build
```
