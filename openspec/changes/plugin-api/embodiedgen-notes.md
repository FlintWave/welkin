# EmbodiedGen Research Notes

**Date:** 2026-11-15
**Status:** Research only, no resources allocated

## What EmbodiedGen actually is

EmbodiedGen is not a simple text-to-3D mesh tool. It is Horizon Robotics'
open-source **3D world engine for embodied AI**: an agentic pipeline that
produces simulation-ready worlds with metric geometry, physical properties,
collision meshes, affordance annotations, and standardized simulator output.

The V2 paper (arXiv 2607.07459, July 2026) describes it as a two-level system:

**Object level:** Each generated asset includes:
- Textured visual geometry
- Convex collision proxy (for physics)
- VLM-inferred physical properties (scale, mass, friction)
- Affordance annotations (how the object can be interacted with)
- Output as URDF, MJCF, or USD

**Scene level (V2):** Generates multi-room and whole-house scenes with:
- Room topology and spatial layout
- Individually addressable furniture and objects
- Task-level affordances (for robotics training)

The original V1 (2025) listed six modules: Image-to-3D, Text-to-3D, Texture
Generation, Articulated Object Generation, Scene Generation, and Layout
Generation.

## System requirements

The README does not publish minimum VRAM or GPU specs. What is known:

- **Python 3.10.13** in a conda environment
- **CUDA toolkit**: cu126 (RTX 40-series default), cu128 (RTX 50-series/Blackwell)
- **Install**: `bash install.sh basic` (~10 minutes); `bash install.sh extra`
  adds the scene generation CLI (`scene3d-cli`)
- **LLM backend required**: most pipelines need an LLM agent (Azure OpenAI,
  OpenRouter with API key, or Codex CLI)
- **Model checkpoints**: downloaded automatically on first run (significant
  disk space, likely 10-50 GB given similar projects)
- **Dev hardware hint**: commit metadata shows maintainers used 4090 machines
  (24 GB VRAM), suggesting that is the development baseline

Given that it runs Stable Diffusion 3.5 Medium and 3D reconstruction models,
a practical minimum is likely 16-24 GB VRAM. The user's RTX 4070 SUPER (12 GB)
may be tight for the full pipeline; the RTX 3070 (8 GB) would almost certainly
be insufficient for anything beyond basic object generation.

## License

Apache 2.0 (compatible with Welkin's licensing)

## How it could connect to Welkin

EmbodiedGen outputs URDF and mesh files. Welkin would need:

1. A conversion step from URDF/mesh to OpenSim-compatible format (Collada .dae
   or glTF, depending on which pipeline is wired)
2. A server-side module that invokes EmbodiedGen's CLI, waits for output, and
   imports the result via Welkin's mesh upload or prim XML path
3. The llm-world-copilot module could use EmbodiedGen as one of its mesh
   generation backends, alongside Meshy and Modly

The scene generation capability is the most interesting part for a virtual world
server. It could generate entire furnished rooms or buildings and import them
as linked object sets. That goes well beyond single-object generation.

## Caveats

- VRAM requirements are not documented; test before committing to this path
- The LLM agent dependency means running EmbodiedGen adds both GPU cost and
  API cost (unless using a local LLM)
- V2 is very recent (July 2026); the API surface may change
- Some users may object to AI-generated content in shared virtual worlds

## Sources

- [GitHub: HorizonRobotics/EmbodiedGen](https://github.com/HorizonRobotics/EmbodiedGen)
- [EmbodiedGen V2 paper (arXiv 2607.07459)](https://arxiv.org/pdf/2607.07459)
- [EmbodiedGen V1 paper (arXiv 2506.10600)](https://arxiv.org/abs/2506.10600)
- [HuggingFace collection](https://huggingface.co/collections/HorizonRobotics/embodiedgen)
- [Project documentation](https://horizonrobotics.github.io/EmbodiedGen)
