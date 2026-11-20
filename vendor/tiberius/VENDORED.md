# tiberius 0.12.3 (vendored)

Copy of the published `tiberius` 0.12.3 crate (MIT/Apache-2.0), used through `[patch.crates-io]` in the root
`Cargo.toml`. The only change is support for SQL Server collations on DOS code pages, which `encoding_rs`
cannot decode and made every query with such a `varchar` column fail with
`encoding: unspported encoding (LCID: 0x409, sort ID: 44)`:

- `src/tds/oem.rs` (new): CP437 and CP850 tables and decoder.
- `src/tds/collation.rs`: `Collation::oem_table()` (sort IDs 30-35 -> CP437; 40-45, 49, 55-61 -> CP850).
- `src/tds/codec/column_data/string.rs` and `text.rs`: decode with the OEM table when there is one.

Second change, TLS for old SQL Server installations (2008/2008 R2/2012 without the TLS updates), which only offer
TLS 1.0/1.1 or RSA key exchange and close rustls connections with `tls handshake eof`:

- `Cargo.toml`: feature `legacy-openssl` (implies `rustls`) with the optional `openssl` (feature `vendored`, a static
  OpenSSL 3 built by `openssl-src`, so the result does not depend on the system library or its TLS policy) and
  `tokio-openssl` dependencies.
- `src/client/tls_stream/openssl_legacy_stream.rs` (new): with that feature `TlsStream` is an enum of the rustls
  stream and an OpenSSL stream (TLS 1.0 minimum, `ALL:@SECLEVEL=0`, legacy server connect, no certificate check).
- `src/client/tls_stream.rs`: the `cfg`s pick that module when `legacy-openssl` is on.
- `src/client/config.rs`: `Config::legacy_tls(bool)` chooses OpenSSL for the connection; default false (rustls).

Also `#![allow(deprecated)]` in `src/lib.rs`, since cargo shows the warnings of path dependencies.

Removed from the copy: `rust-toolchain`, `docker*`, `flake.*` (development files of the upstream repository).
When upgrading tiberius, reapply these changes or drop the patch if upstream supports these code pages.
