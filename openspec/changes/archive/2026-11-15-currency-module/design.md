# Currency Module — Design

## Architecture

```
Viewer <--LLUDP--> Region Server <--trait--> MoneyModule impl
                       |                         |
                       +-- /currency.php ----+    +-- SampleMoneyModule (default, no-op)
                       +-- /landtool.php ----+    +-- DTLProxyModule (forwards to MoneyServer)
                       +-- SimulatorFeatures      +-- GloebitModule (REST API client)
                       |
                       +-- LSL: llGiveMoney, llSetPayPrice, money() event
```

## MoneyModule trait

Rust equivalent of `IMoneyModule`. Registered as a region module. Only one
active money module per region (selected by config).

```rust
pub trait MoneyModule: Send + Sync {
    fn object_give_money(&self, object: Uuid, from: Uuid, to: Uuid, amount: i32, txn: Uuid) -> bool;
    fn get_balance(&self, agent: Uuid) -> i32;
    fn upload_covered(&self, agent: Uuid, amount: i32) -> bool;
    fn amount_covered(&self, agent: Uuid, amount: i32) -> bool;
    fn apply_charge(&self, agent: Uuid, amount: i32, tx_type: MoneyTransactionType, extra: &str);
    fn apply_upload_charge(&self, agent: Uuid, amount: i32, text: &str);
    fn move_money(&self, from: Uuid, to: Uuid, amount: i32, text: &str);
    fn move_money_typed(&self, from: Uuid, to: Uuid, amount: i32, tx_type: MoneyTransactionType, text: &str) -> bool;
    fn upload_charge(&self) -> i32;
    fn group_creation_charge(&self) -> i32;
}
```

## Economy constants (EconomyData packet)

Sent to viewer on login. All values from OpenSim's SampleMoneyModule:

| Field | Value | Purpose |
|-------|-------|---------|
| ObjectCapacity | 15000 | Max prims per region |
| ObjectCount | (live) | Current prim count |
| PriceEnergyUnit | 0 | Unused |
| PriceObjectClaim | 0 | Cost to claim object |
| PricePublicObjectDecay | 0 | Decay rate |
| PricePublicObjectDelete | 0 | Delete cost |
| PriceParcelClaim | 0 | Land claim cost |
| PriceParcelClaimFactor | 1.0 | Claim multiplier |
| PriceUpload | 0 | Upload cost (overridden by money module) |
| PriceRentLight | 0 | Light rental |
| TeleportMinPrice | 0 | Teleport cost |
| TeleportPriceExponent | 0 | Distance multiplier |
| EnergyEfficiency | 0.0 | Unused |
| PriceObjectRent | 0.0 | Object rental |
| PriceObjectScaleFactor | 0.0 | Scale cost factor |
| PriceParcelRent | 0 | Land rental |
| PriceGroupCreate | 0 | Group creation cost (overridden by money module) |

## XML-RPC endpoints

Both serve on the region HTTP port, matching OpenSim's legacy paths:

### /currency.php
- `getCurrencyQuote` — returns `{ currency: { estimatedCost: "<$0.00>", currencyBuy: amount }, confirm: "..." }`
- `buyCurrency` — SampleMoneyModule returns success (no real purchase)

### /landtool.php
- `preflightBuyLandPrep` — returns membership level, land use info, currency balance
- `buyLandPrep` — returns success (no real purchase)

## DTL/NSL proxy mode

Config-driven. When `[Economy] CurrencyServer = http://host:8008/`, all
MoneyModule methods forward to the external MoneyServer via XML-RPC. The
`/currency.php` and `/landtool.php` endpoints also proxy. This lets existing
DTL/NSL deployments work unmodified.

## Gloebit client

Native Rust HTTP client calling the Gloebit REST API. Config requires
`[Gloebit] GLBKey` and `GLBSecret`. Handles OAuth2 authorization flow,
balance queries, and transfers. Hypergrid-aware (user identity from home grid).

## Decisions

1. Plugin interface is trait-based (not WASM), selected by config string.
   Grid operators choose `sample`, `dtl`, or `gloebit` in `[Economy]`.
2. SampleMoneyModule is the default — zero cost for everything, matches
   OpenSim behavior for grids without commerce.
3. Economy constants are configurable via `[Economy]` section, not hardcoded.
