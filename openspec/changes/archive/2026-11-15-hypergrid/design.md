# hypergrid: Cross-grid federation

## Summary

Port the Hypergrid (HG) system from OpenSimulator 0.9.3.0: ~11,171 lines of
C# across ~23 files in two layers (Robust services and region modules). HG
enables cross-grid teleportation, inventory transfer, friend lists, IM, and
map linking between independent OpenSimulator grids. Most HG modules extend
already-ported base modules.

The system has two sides: the **Gatekeeper** (entry point for foreign
visitors) and the **UserAgent** service (manages outbound travel from the
home grid). Supporting services handle inventory, assets, friends, and IM
across grid boundaries.

## Scope

### In scope

- **GatekeeperService** (725 lines): foreign visitor entry, hyperlink creation,
  agent login verification, AllowedClients/DeniedClients regex filtering,
  foreign agent allow/deny lists.
- **UserAgentService** (754 lines): outbound HG teleport, traveling agent
  tracking, session verification (`VerifyAgent`/`VerifyClient`), Universal
  User Identifier (UUI) format, home region lookup.
- **HGSuitcaseInventoryService** (654 lines): restricted "suitcase" inventory
  for foreign visitors with path validation to prevent escape.
- **HGFriendsService** (408 lines): cross-grid friend management with secret
  verification, online/offline status across grids.
- **HG asset services** (720 lines combined): `HGAssetService`,
  `HGFSAssetService`, `HGRemoteAssetService` — access-controlled asset
  transfer between grids.
- **HGInstantMessageService** (301 lines): cross-grid IM routing via
  UserAgentService location resolution.
- **HypergridLinker** (799 lines): grid-to-grid region linking, console
  commands (`link-region`/`unlink-region`), link persistence.
- **Service connectors** (1,389 lines): HTTP clients for Gatekeeper,
  UserAgent, HGFriends, and HELO probe.
- **Region modules** (3,577 lines): HGEntityTransferModule, HGFriendsModule,
  HGInventoryBroker, HGInventoryAccessModule, HGAssetMapper,
  HGMessageTransferModule, HGLureModule, HGWorldMapModule,
  HGUserManagementModule, HypergridServiceInConnectorModule.

### Out of scope

- HG group federation (covered by social-modules HGGroupsService).
- Cross-grid profile synchronization (read-only in C#, low priority).
- Automated grid directory (no C# equivalent exists).

## Dependencies

All base modules already ported:

- EntityTransferModule (region-modules section 11)
- FriendsModule (region-modules section 10)
- Inventory services (grid-services)
- MessageTransferModule (region-modules section 7)
- WorldMapModule (region-modules section 8)
- UserManagementModule (region-modules section 12)
- Asset services (grid-services)
- Grid services (grid-services)

## Design

### Architecture

```
Foreign Grid                          Home Grid
┌─────────────┐   HTTP/XMLRPC   ┌─────────────────┐
│ UserAgent    │ ←──────────────→│ Gatekeeper       │
│ Service      │                 │ Service          │
└─────────────┘                  └────────┬────────┘
                                          │
                                 ┌────────▼────────┐
                                 │ Region Simulator │
                                 │ HGEntityTransfer │
                                 │ HGInventoryAccess│
                                 │ HGFriends        │
                                 │ HGAssetMapper    │
                                 └─────────────────┘
```

HG region modules extend their base counterparts (trait inheritance or
wrapper delegation). The Gatekeeper controls inbound access; the UserAgent
controls outbound travel. Session tokens are exchanged during teleport and
verified on arrival.

### Key decisions

- H1: HG modules extend base modules via trait composition, not separate
  implementations. `HGEntityTransferModule` wraps `EntityTransferModule` and
  adds HG-specific logic (appearance restriction, inventory transfer).
- H2: UUI format (`uuid;griduri;name`) stored alongside local UUIDs. The
  `HGUserManagementModule` resolves `first.last@griduri` to local cache.
- H3: Suitcase inventory uses path validation with a root-anchored check to
  prevent directory traversal to the full inventory tree.
- H4: Traveling agent sessions use cryptographically random tokens, not
  predictable values. Tokens are single-use and expire on logout.
- H5: HELO probe (`HeloServicesConnector`) discovers HG capability before
  attempting cross-grid operations, with timeout and retry.

## Security considerations

- **Suitcase escape** (CRITICAL): HGSuitcaseInventoryService must prevent
  foreign visitors from accessing inventory outside their suitcase folder.
  Path validation is defense in depth over folder-ID checks.
- **Asset export** (HIGH): HG asset services must respect transfer permissions.
  No-transfer assets must not be copied to foreign grids.
- **Session token forgery** (HIGH): VerifyAgent/VerifyClient tokens must be
  cryptographically strong and single-use. Replay attacks must be prevented.
- **Agent spoofing** (MEDIUM): Gatekeeper must verify incoming agent identity
  against the claimed home grid's UserAgentService.
- **DDoS via hyperlinks** (MEDIUM): Rate-limit hyperlink creation and foreign
  agent login attempts.
