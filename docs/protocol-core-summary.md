# protocol-core summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md) · Previous: [Phase 0](phase-0-summary.md) · Next: [Physics backend](physics-backend-summary.md)

Change `protocol-core` (accepted; all tasks ticked). It replaces
libopenmetaverse and the C# `Region/ClientStack` protocol layer with wrapped
Aurora crates and new server-side code.

## 1. Reuse (`55ffecf`, `f31913e`)

`aurora-llsd`, `aurora-msg` and `aurora-prim` are pinned at one commit
(`3d523b1`) and exposed through `welkin-llsd`, `welkin-msg` and
`welkin-prim`. An inventory recorded which messages and structures the
Aurora crates can read, write, or only one of.

## 2. Types and encodings

- `welkin-types` (`56249e5`, merged `62c23be`): asset, inventory, folder and
  wearable type enums, attachment points, `PermissionMask`; 22 tests.
  Vectors and quaternions are `glam`.
- XML-RPC (`27d5fe3`, merged `7ff0cb0`): server-side encode and decode for
  the login flow, in `welkin-llsd/src/xmlrpc.rs`.
- LLUDP roundtrip (`1e33953`, merged `5a921db`):
  `welkin-conformance/tests/lludp_roundtrip.rs` decodes and re-encodes every
  packet of a recorded C# session without error.

## 3. Server-direction gaps

- Object update encoders (`7241c49`): `welkin-msg/src/objects.rs` provides
  `encode_terse`, `encode_object_data` (full) and `encode_compressed`, with
  `FullObjectUpdate`. Aurora only decodes these.
- Decoders for client-to-server messages: already provided by `aurora-msg`
  (`4773680`), so nothing was added.
- Upstream offer: no changes were needed beyond the encoders (`6ea702a`).

## 4. Circuit (`welkin-net`)

- `circuit.rs` (`6bdd9b7`): server-side LLUDP circuit with sequence numbers,
  reliable delivery, resend (max 4 retries), acks flushed every 100 ms,
  duplicate detection (2048-packet window), 300 ms default RTT.
- `throttle.rs` (`3833feb`): seven categories (Resend, Land, Wind, Cloud,
  Task, Texture, Asset) with a token bucket each, modelled on `LLUDPClient`.
- Ping and timeout handling (`43234da`).

## 5. Proof

- Test bot on `aurora-net` (`ee43e40`, `51d1811`):
  `welkin-conformance/tests/test_bot.rs` has `test_bot_circuit_handshake`
  and `test_bot_login_handshake`, so the server sees the same requests the
  viewer makes.
- Fuzz targets (`99b6a0e`): `crates/welkin-msg/fuzz/fuzz_targets/`
  `fuzz_parse_packet.rs` and `fuzz_object_updates.rs`.

## Design choices

- Wrap every Aurora crate; update the pin only through a reviewed change gated
  by replay tests.
- `aurora-net` is test-only; the server circuit is its own implementation
  because Aurora's is the viewer side.
- Throttle and resend behaviour follows the C# server closely, since Aurora
  matches the reference viewer's rates and a mismatch would stall or flood.

## Caveats

- Task 5.2 says the bot handshakes with the C# simulator; the committed tests
  should be checked for whether they run against the reference container or
  the Rust circuit.
- The change has no `design.md`; rationale is in `proposal.md`.
