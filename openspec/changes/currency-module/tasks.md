# currency-module tasks

## Phase 1: Core contract and viewer protocol

- [x] Define MoneyModule trait matching IMoneyModule (8 methods + upload_charge + group_creation_charge)
- [x] Define MoneyTransactionType enum (ClassifiedCharge, GroupCreate, GroupJoin, LandPassSale, etc.)
- [x] Implement SampleMoneyModule (zero-balance, allow-all, matches OpenSim default)
- [x] Register money module in region server config system (select by name)
- [x] Handle EconomyDataRequest — send EconomyData with 16 economy constants
- [x] Handle MoneyBalanceRequest — query money module, send MoneyBalanceReply
- [x] Handle MoneyTransferRequest — validate via money module, send MoneyBalanceReply result
- [x] Handle RequestPayPrice — query object pay price, send PayPriceReply
- [x] Handle ObjectBuy — validate purchase via money module, transfer object
- [x] Implement /currency.php XML-RPC endpoint (getCurrencyQuote, buyCurrency)
- [x] Implement /landtool.php XML-RPC endpoint (preflightBuyLandPrep, buyLandPrep)
- [x] Add currency-base-uri to SimulatorFeatures cap OpenSimExtras
- [x] Add currency field to login response
- [x] Wire EventManager hooks: OnMoneyTransfer, OnObjectPaid, OnValidateLandBuy, OnLandBuy

## Phase 2: LSL integration

- [x] Implement llGiveMoney (requires PERMISSION_DEBIT grant)
- [x] Implement llTransferLindenDollars (returns transaction UUID)
- [x] Implement llSetPayPrice (sets pay dialog buttons on object)
- [x] Implement money() script event (fired via OnObjectPaid)

## Phase 3: External system support

- [x] Implement DTL/NSL proxy module (forward all money calls to CurrencyServer URL via XML-RPC)
- [x] Implement Gloebit client module (OAuth2 flow, REST API for balance/transfer)
- [x] Add Gloebit config section (GLBKey, GLBSecret, sandbox mode)
- [x] Test DTL/NSL proxy against reference MoneyServer.exe
- [x] Test Gloebit module against Gloebit sandbox API
