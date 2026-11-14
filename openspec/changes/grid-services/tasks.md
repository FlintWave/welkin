## 1. Data layer

- [x] 1.1 `sqlx` access for each store, three database backends
- [x] 1.2 Carry over migrations; verify a C#-created database opens unchanged
- [x] 1.3 Record C# service traffic for every handler with the conformance recorder

## 2. Services

- [x] 2.1 Assets and file-system asset store
- [x] 2.2 Inventory
- [x] 2.3 Grid, presence, grid user
- [x] 2.4 User accounts, authentication, authorization
- [x] 2.5 Friends, avatar, mute list, user profiles
- [x] 2.6 Map images, estate, simulation
- [x] 2.7 Login

## 3. Proof

- [ ] 3.1 Replay tests pass for every handler
- [ ] 3.2 A C# simulator runs against the Rust `robust` and `pCampBot` logs in, rezzes and teleports
- [ ] 3.3 Aurora Viewer logs in to the mixed grid
- [ ] 3.4 Second review of authentication, inventory writes and asset writes
