# tiberius 0.12.3 (vendored)

Copy of the published `tiberius` 0.12.3 crate (MIT/Apache-2.0), used through `[patch.crates-io]` in the root
`Cargo.toml`. The only change is support for SQL Server collations on DOS code pages, which `encoding_rs`
cannot decode and made every query with such a `varchar` column fail with
`encoding: unspported encoding (LCID: 0x409, sort ID: 44)`:

- `src/tds/oem.rs` (new): CP437 and CP850 tables and decoder.
- `src/tds/collation.rs`: `Collation::oem_table()` (sort IDs 30-35 -> CP437; 40-45, 49, 55-61 -> CP850).
- `src/tds/codec/column_data/string.rs` and `text.rs`: decode with the OEM table when there is one.

Also `#![allow(deprecated)]` in `src/lib.rs`, since cargo shows the warnings of path dependencies.

Removed from the copy: `rust-toolchain`, `docker*`, `flake.*` (development files of the upstream repository).
When upgrading tiberius, reapply these changes or drop the patch if upstream supports these code pages.
