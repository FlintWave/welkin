# Terrain LayerData golden vectors (from OpenSim 0.9.3.0)

Purpose: regression fixtures for `welkin-scene::terrain_layer::encode_land_layer` (#51).
Each vector is the byte stream OpenSim's own `TerrainCompressor.cs` produces for a 256x256 height map
(all 256 patches, py outer / px inner, group header stride 264, patch size 16, layer 0x4C, end marker 97).

## Layout
- `manifest.json`: per terrain: generator formula (provenance), input file, OpenSim output file + sha256,
  `compare` mode, reference decode error (LibreMetaverse decode of the OpenSim output), Rust error at 854b115.
- `inputs/<name>.f32.gz`: the height map: gzip of 65536 little-endian f32, row-major `y*256+x`.
- `opensim-bins/<name>.opensim.bin`: the OpenSim output (the expected LayerData bytes).
- `tools/`: how they were made (not meant for the repo, see Licensing).
Total size about 2.0 MB (inputs 1.5 MB gzipped, bins 0.5 MB). Trig/exp inputs are stored because libm results
can differ in the last bit between platforms; the pure-integer-noise ones (noise2m, noise60m, noise1000, lowzero)
could be regenerated from the xorshift seed if size matters.

## Suggested test (Rust, in welkin-scene/tests/terrain_golden.rs)
For each manifest entry: gunzip the input into a `TerrainChannel` (set(x,y,v)), call `encode_land_layer(&t, &all_patches)`.
- `compare == "bytes_identical"` (17 terrains): assert the output equals `opensim_output` byte for byte.
- `compare == "decode_only"` (`ramp`, `noise2m`): OpenSim and Rust differ by rounding ties in a quantised coefficient
  (48 and 1 bytes respectively at 854b115); assert `decode_land_layer(output)` max/mean error versus the input
  is <= `reference_decode_error * 1.01 + 0.001`.
Also assert `decode_land_layer(opensim_output)` reproduces the same error figures (checks the crate decoder against OpenSim data).
Error figures use `max = max |decoded - input|` over all 65536 cells, patch height index `y*16+x` (verified orientation).
The errors for noisy or spiky terrain are large by design (lossy format); they are the same in OpenSim.

## Provenance and checks done (2026-11-16, Welkin 854b115)
- OpenSim output: OpenSim's own `dct16x16`, quantiser, copy matrix, `EncodePatch` and header code, compiled in a C# harness
  with a corrected patch min/max (OpenSim's `GetPatchMinMax` mis-reports the minimum when values keep rising within a patch).
- Decode check: Rust and OpenSim outputs were decoded with LibreMetaverse 3.3.0 `TerrainManager.DecompressLand`; all 256 patches
  decoded for all 19 terrains.

## Licensing
The `.bin` files are data produced by running OpenSim code. `tools/opensim-ref-harness/make_ref.py` extracts
OpenSim (BSD-3) code into `Ref.cs` at build time; do not commit `Ref.cs` (the project states it contains no OpenSim code).
Only the vectors, the manifest and, if wanted, the Rust generators and decode harness (own code) are suitable for the repo.
