## 1. Voice module foundation

- [ ] 1.1 IVoiceModule trait with `set_land_sip_address`, WebRtcVoiceModule skeleton as ISharedRegionModule, INI config (wolfvoice endpoint, TURN server, defaults)
- [ ] 1.2 wolfvoice JSON-RPC client: connection, provision_account, create_channel, destroy_channel, health check

## 2. Voice capabilities

- [ ] 2.1 ProvisionVoiceAccountRequest capability: agent UUID to voice credentials via wolfvoice, LLSD XML response matching viewer expectations
- [ ] 2.2 ParcelVoiceInfoRequest capability: estate voice check, parcel voice check, channel URI construction, LLSD XML response
- [ ] 2.3 Voice channel lifecycle: create channel on first parcel entry, destroy on empty, handle region restart and parcel voice toggle

## 3. Integration

- [ ] 3.1 Wire IVoiceModule to OSSL `osSetParcelSIPAddress`, test that script-driven parcel voice changes propagate
- [ ] 3.2 Docker Compose service definition for wolfvoice sidecar, health check, restart policy, environment variables
- [ ] 3.3 Grid-portal integration: voice service status in admin dashboard, per-estate voice enable toggle

## 4. Testing and security

- [ ] 4.1 Mock wolfvoice JSON-RPC server for integration tests: provision, channel create/destroy, error responses
- [ ] 4.2 Integration tests: provision flow, parcel crossing channel switch, estate voice disable, concurrent provisioning
- [ ] 4.3 SECURITY REVIEW: credential isolation (no cross-agent leakage), JSON-RPC auth (shared secret or mTLS), rate limiting, wolfvoice trust boundary
