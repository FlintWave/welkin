# Compute Rental Revenue Model — Research Note

**Status:** Research only, no resources allocated
**Date:** 2026-11-15

## Concept

Run hosted Welkin servers rather than publishing the source immediately. Offer
GPU-intensive features (EmbodiedGen V2, future AI tools) as paid add-ons so
users don't need expensive hardware.

## Precedents: Virtual World Hosting

OpenSim grid hosting runs $5–30/month per region depending on prim count and
provider. Typical 15,000-prim region averages ~$13–20/month. Kitely charges
$50–100/month for premium worlds. Second Life's land fees are higher ($7–175+
depending on tier). These are CPU-bound workloads with no GPU requirement.

No existing virtual world platform bundles AI generation as a paid service
tier. This would be a first-mover position.

## GPU Compute Costs (our cost to run)

RTX 4090 (24GB VRAM, sufficient for EmbodiedGen): $0.33–0.74/hr on-demand
from cloud providers (median ~$0.48/hr as of Oct 2026). Spot pricing drops to
~$0.16/hr. RTX A6000 (48GB): $0.29–0.93/hr.

A single EmbodiedGen generation takes roughly 1–3 minutes of GPU time. At
$0.48/hr, that's $0.008–0.024 per generation in compute cost.

## What We Could Charge

AI 3D generation services use credit models: Masterpiece X charges ~$0.20 per
model (50 credits from a 750-credit/$14.99 pack). SimpleMesh sells 30
generations for $9 (~$0.30 each). AI World Generator charges $9.90/month for
500 credits.

A reasonable price point: $0.25–0.50 per generation, or a subscription
bundling N generations/month into the hosting fee. At $0.25/generation with
$0.02 compute cost, gross margin is ~92%.

## Revenue Structure Options

1. **Hosting + generation bundles:** $25–50/month for a region with N
   included generations, overage at $0.25–0.50 each
2. **Pure add-on:** free/cheap hosting, pay per generation
3. **Tiered:** free tier with limited generations, paid tiers unlock more

## Risks

- **Apache 2.0 on EmbodiedGen:** anyone can run the same service. Competitive
  moat is operational (good infra, low latency, integrated UX), not legal.
- **Self-hosting competition:** power users with 24GB+ GPUs can run it
  themselves. The target market is users without that hardware.
- **Open-source community tension:** OpenSim's community expects open source.
  A proprietary-hosted model may face pushback. Mitigations: open-source
  Welkin itself, keep the paid layer as an optional managed service on top.
  "Open core" model: server is open, hosted AI features are the paid tier.
- **GPU cost volatility:** cloud GPU prices have dropped ~40% in 2025-2026
  and may continue falling, compressing margins if competitors undercut.

## Unit Economics Summary

| Item | Cost | Price | Margin |
|------|------|-------|--------|
| Region hosting (CPU) | ~$3–5/mo | $15–25/mo | 70–80% |
| Single 3D generation | ~$0.02 | $0.25–0.50 | 90–96% |
| 100 generations/mo | ~$2.00 | $15–25/mo | 87–92% |

## Conclusion

The margins are strong, the market has no direct competitor offering AI
generation inside a virtual world hosting service, and the open-core model
(open server, paid AI tier) avoids antagonizing the open-source community.
Hardware cost is the main barrier to self-hosting, which is exactly what makes
a managed service attractive. Worth deeper analysis if Welkin reaches a
releasable state and the plugin API is built.
