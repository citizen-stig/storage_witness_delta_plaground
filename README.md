# Sandbox for simulating snapshot-based STF with Reorgs

## Types

### sovereign-sdk simplified types:

* `Key`, `Value` : simplified version of StorageKey and StorageValue

### New Traits

We assume to have 2 new traits: Snapshot and ForkTreeManager

```rust
```

Only *`Snapshot`* will be visible for STF.

`ForkTreeManager` is going to live under "node/services". This will allow to have stf-runner remain generic enough to
not depend on the module system, but only on interface

### New Types

### Modification of existing types

