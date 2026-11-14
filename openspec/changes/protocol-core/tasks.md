## 1. Reuse

- [x] 1.1 Add `aurora-llsd`, `aurora-msg` and `aurora-prim` as git dependencies pinned to one commit; confirm they build outside Aurora's workspace
- [x] 1.2 Inventory each crate from the server's side: which messages and structures can be both read and written, and which only one way
- [x] 1.3 Wrapper crates exposing only what the server uses

## 2. Types and encodings

- [ ] 2.1 `types` crate for what Aurora does not define, with round-trip tests against values produced by the C# server
- [x] 2.2 XML-RPC encode and decode
- [ ] 2.3 Decode and re-encode every packet in a recorded C# session without error

## 3. Server-direction gaps

- [ ] 3.1 Encoders for object updates (full, terse, compressed) and other messages the viewer only decodes
- [ ] 3.2 Decoders for messages the viewer only sends
- [ ] 3.3 Offer the additions to Aurora Viewer upstream

## 4. Circuit

- [ ] 4.1 Server circuit: open, close, sequence numbers, acknowledgements, resend
- [ ] 4.2 Throttle categories and token buckets as in `LLUDPClient`
- [ ] 4.3 Ping and timeout handling

## 5. Proof

- [ ] 5.1 Test bot on `aurora-net`
- [ ] 5.2 The bot completes a login and circuit handshake with the C# simulator
- [ ] 5.3 Fuzz targets for the server-side decoders
