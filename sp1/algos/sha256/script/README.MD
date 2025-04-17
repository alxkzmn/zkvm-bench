## RAM Usage Measurement for SHA256

1. Build the project (generates the sha binary in ../target/release):

```
cargo build --release
```

2. Run a "no-measure" setup to generate the proving key (pk.bin), verifying key (vk.bin), and proof (proof.bin):

```
# Generate and write pk.bin and vk.bin:
cargo run --release -- --setup
# Generate and write proof:
cargo run --release -- --prove
```

3. Use the monitor script to measure RAM usage:

```
sh monitor.sh setup-no-write
```

and

```
sh monitor.sh prove-no-write
```

## Benchmarking

```
cargo bench
```
