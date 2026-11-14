## 1. Gatekeeper service

- [ ] 1.1 IGatekeeperService trait, GatekeeperService: LinkLocalRegion (hyperlink creation), GetHyperlinkRegion (redirect/allow), access control (AllowedClients/DeniedClients regex, foreign agent allow/deny)
- [ ] 1.2 GatekeeperService LoginAgent: verify foreign agent, create local presence, session token exchange, BansService integration

## 2. UserAgent service

- [ ] 2.1 IUserAgentService trait, UserAgentService: LoginAgentToGrid (outbound HG teleport), LogoutAgent, GetHomeRegion, GetServerURLs, traveling agent DB tracking
- [ ] 2.2 Session verification: VerifyAgent/VerifyClient with cryptographic tokens, GetUUI (Universal User Identifier), LocateUser

## 3. HG inventory

- [ ] 3.1 HGInventoryService: proxied inventory access for HG visitors, ownership validation
- [ ] 3.2 HGSuitcaseInventoryService: suitcase folder restriction, root-anchored path validation, GetRootFolder override, folder creation controls

## 4. HG assets and friends

- [ ] 4.1 HGAssetService + HGFSAssetService + HGRemoteAssetService: access-controlled asset transfer, transfer permission enforcement, no-transfer asset blocking
- [ ] 4.2 HGFriendsService + HGFriendsModule: cross-grid friendship with secret verification, online/offline status notification across grids, extends FriendsModule

## 5. HG messaging

- [ ] 5.1 HGInstantMessageService: cross-grid IM routing, user location resolution via UserAgentService, delivery retry
- [ ] 5.2 HGMessageTransferModule: routes IMs to local or remote grid based on user home, extends MessageTransferModule

## 6. Hyperlink management

- [ ] 6.1 HypergridLinker: grid-to-grid region linking, link-region/unlink-region console commands, link persistence in grid service, map coordinate assignment
- [ ] 6.2 HeloServicesConnector: HELO probe for HG capability discovery, timeout handling

## 7. Service connectors

- [ ] 7.1 GatekeeperServiceConnector + UserAgentServiceConnector: HTTP clients matching C# Robust endpoints, request/response serialization
- [ ] 7.2 HGFriendsServicesConnector: HTTP client for cross-grid friend operations

## 8. HG region modules

- [ ] 8.1 HGEntityTransferModule: extend EntityTransferModule for cross-grid teleport, appearance restriction (clothing validation), inventory transfer on HG crossing, foreign agent verification
- [ ] 8.2 HGInventoryAccessModule + HGInventoryBroker: route inventory requests to local or remote service based on user's home grid, suitcase enforcement on rez/take
- [ ] 8.3 HGLureModule + HGWorldMapModule + HGUserManagementModule: cross-grid teleport lures, map blocks for hyperlinked regions, UUID-to-name resolution for foreign users (first.last@griduri)

## 9. Integration and connector module

- [ ] 9.1 HypergridServiceInConnectorModule: expose HG service endpoints within the simulator for incoming requests from foreign grids

## 10. Testing and security

- [ ] 10.1 Integration tests: HG teleport round-trip (outbound+inbound), suitcase inventory access, cross-grid IM delivery, hyperlink creation/removal, friend request across grids
- [ ] 10.2 SECURITY REVIEW: suitcase folder escape, asset export permission enforcement, session token strength and replay prevention, agent identity spoofing, rate limiting (CRITICAL RISK, Opus reviewer)
