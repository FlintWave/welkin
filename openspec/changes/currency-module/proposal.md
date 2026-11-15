# Currency Module

## Summary

Implement the `IMoneyModule` interface contract and viewer-facing economy protocol
so that existing OpenSim currency systems (Gloebit, DTL/NSL Money Server, Podex)
work with Welkin grids without modification to their server-side components.

## Motivation

Every production OpenSim grid that handles commerce uses one of two currency
systems: Gloebit (cloud-hosted, hypergrid-enabled) or DTL/NSL Money Server
(self-hosted, MySQL-backed). Both integrate through OpenSim's `IMoneyModule`
interface and the viewer economy protocol (5 UDP packets, 4 XML-RPC methods,
SimulatorFeatures cap, login response field). Without this contract, grids
running Welkin cannot enable commerce.

## Scope

### In scope

- `MoneyModule` Rust trait matching `IMoneyModule` (8 methods + events)
- `SampleMoneyModule` default (zero-balance, allow-all — matches OpenSim)
- UDP packet handlers: EconomyDataRequest, MoneyBalanceRequest,
  MoneyTransferRequest, RequestPayPrice, ObjectBuy
- UDP response packets: EconomyData (16 economy constants),
  MoneyBalanceReply, PayPriceReply
- XML-RPC endpoints: `/currency.php` (getCurrencyQuote, buyCurrency),
  `/landtool.php` (preflightBuyLandPrep, buyLandPrep)
- `currency-base-uri` in SimulatorFeatures cap OpenSimExtras
- `currency` field in login response
- LSL functions: llGiveMoney, llTransferLindenDollars, llSetPayPrice
- LSL event: money()
- EventManager hooks: OnMoneyTransfer, OnObjectPaid, OnValidateLandBuy, OnLandBuy
- Plugin interface for external money modules (Rust dylib or config-driven proxy)
- DTL/NSL proxy mode (forward to external MoneyServer URL)
- Native Gloebit REST client

### Out of scope

- Building a currency system (grid operator decision)
- OMC/VirWoX support (defunct)
- Custom payment processing or real-money handling

## Phase

This belongs in the region-modules phase (phase 5) or as a standalone change
after it. The trait and SampleMoneyModule can land first, with DTL/NSL proxy
and Gloebit client as follow-ups.

## Risk

Low. The interface is small and well-defined. The SampleMoneyModule is trivial.
External system compatibility depends on matching the XML-RPC protocol exactly —
use the C# SampleMoneyModule and DTL/NSL source as references.
