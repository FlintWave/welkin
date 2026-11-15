# Currency module summary

[← Back to docs](README.md) · [Architecture](architecture.md)

Status: accepted (23 of 23 tasks, 3 phases). Change:
`openspec/changes/currency-module/`.

## What was built

The economy system that lets existing OpenSim currency providers (Gloebit,
DTL/NSL Money Server, Podex) work with Welkin grids. The implementation
covers the `IMoneyModule` interface contract, five UDP packet handlers, four
XML-RPC methods, three LSL money functions, and native clients for the two
major currency systems.

Grid operators choose a money module by name in config: `sample` (default,
zero-cost everything), `dtl` (proxy to external MoneyServer), or `gloebit`
(native REST client with OAuth2).

## Architecture

| Module | Responsibility |
|---|---|
| `MoneyModule` trait | Rust equivalent of `IMoneyModule`: 8 methods plus `upload_charge` and `group_creation_charge` |
| `MoneyTransactionType` | Enum of transaction categories (ClassifiedCharge, GroupCreate, LandPassSale, etc.) |
| `SampleMoneyModule` | Default: zero balance, allow all transactions, matches C# OpenSimulator behavior |
| EconomyData handler | Sends 16 economy constants to the viewer on request |
| MoneyBalanceRequest handler | Queries money module, sends MoneyBalanceReply |
| MoneyTransferRequest handler | Validates via money module, sends balance result |
| RequestPayPrice handler | Queries object pay price, sends PayPriceReply |
| ObjectBuy handler | Validates purchase, transfers object ownership |
| `/currency.php` | XML-RPC: getCurrencyQuote and buyCurrency |
| `/landtool.php` | XML-RPC: preflightBuyLandPrep and buyLandPrep |
| SimulatorFeatures | `currency-base-uri` in OpenSimExtras |
| LSL bindings | llGiveMoney, llTransferLindenDollars, llSetPayPrice, money() event |
| EventManager hooks | OnMoneyTransfer, OnObjectPaid, OnValidateLandBuy, OnLandBuy |
| DTL/NSL proxy | Forwards all money calls to external CurrencyServer URL via XML-RPC |
| Gloebit client | Native Rust HTTP client: OAuth2 flow, balance queries, transfers, hypergrid-aware |

## Key decisions

1. Plugin interface is trait-based, selected by config string. No WASM or
   dynamic loading needed.
2. SampleMoneyModule is the default for grids without commerce. Zero cost
   for everything, matching OpenSimulator behavior exactly.
3. Economy constants (16 fields in EconomyData) are configurable via the
   `[Economy]` config section, not hardcoded.
4. DTL/NSL proxy mode forwards to the existing MoneyServer.exe without
   changes on the currency server side.
5. Gloebit client handles the full OAuth2 flow natively in Rust, including
   sandbox mode for testing.

## Tests

DTL/NSL proxy tested against reference MoneyServer.exe. Gloebit module
tested against the Gloebit sandbox API.

## Commits

| Hash | Description |
|---|---|
| `dcc3e8b` | Phase 1: MoneyModule trait, packet handlers, XML-RPC endpoints (14 tasks) |
| `053819d` | Phase 2: LSL money API and money() event (4 tasks) |
| `cc9520a` | Phase 3: DTL/NSL proxy and Gloebit client (5 tasks) |
