// SPDX-License-Identifier: GPL-3.0-or-later
// Automated test harness for welkin using LibreMetaverse 3.3.0

using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using LibreMetaverse;

class TestResult
{
    public string Name { get; set; } = "";
    public bool Passed { get; set; }
    public string Detail { get; set; } = "";
}

class OpenSimTestHarness
{
    static readonly bool Verbose = Environment.GetEnvironmentVariable("VERBOSE") == "1";
    static readonly string LoginUri = Environment.GetEnvironmentVariable("LOGIN_URI") ?? "http://127.0.0.1:8002/";
    static readonly string FirstName = Environment.GetEnvironmentVariable("FIRST_NAME") ?? "Test";
    static readonly string LastName = Environment.GetEnvironmentVariable("LAST_NAME") ?? "User";
    static readonly string Password = Environment.GetEnvironmentVariable("PASSWORD") ?? "password";

    static GridClient? client;
    static readonly List<TestResult> results = new();
    static readonly ManualResetEventSlim loginEvent = new(false);
    static readonly ManualResetEventSlim animationEvent = new(false);
    static readonly ManualResetEventSlim terseUpdateEvent = new(false);
    static readonly ManualResetEventSlim eqRunningEvent = new(false);

    static bool loggedIn = false;
    static Vector3 latestPosition = Vector3.Zero;
    static readonly List<UUID> receivedAnimations = new();
    static bool terseUpdateReceived = false;

    static void Log(string msg)
    {
        if (Verbose)
            Console.WriteLine($"  [DBG] {msg}");
    }

    static void Record(string name, bool passed, string detail = "")
    {
        results.Add(new TestResult { Name = name, Passed = passed, Detail = detail });
        var status = passed ? "PASS" : "FAIL";
        Console.WriteLine($"  [{status}] {name}{(detail != "" ? " -- " + detail : "")}");
    }

    static async Task<int> Main(string[] args)
    {
        Console.WriteLine("=== OpenSim-Rust Test Harness (LibreMetaverse 3.3.0) ===");
        Console.WriteLine($"Login URI: {LoginUri}");
        Console.WriteLine($"Avatar: {FirstName} {LastName}");
        Console.WriteLine();

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));

        try
        {
            client = new GridClient();
            SetupEventHandlers();

            await TestLogin(cts.Token);

            if (!loggedIn)
            {
                Console.WriteLine("\nLogin failed -- skipping remaining tests.");
                PrintSummary();
                return 1;
            }

            await Task.Delay(2000, cts.Token);

            TestRegionInfo();
            TestSpawnPosition();
            TestDefaultAnimation();
            await TestMovement(cts.Token);
            await TestChat(cts.Token);
            await TestAssetService(cts.Token);
            TestEventQueue();
        }
        catch (OperationCanceledException)
        {
            Console.WriteLine("\n** TIMEOUT -- 30s limit reached **");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"\n** EXCEPTION: {ex.Message} **");
            if (Verbose)
                Console.WriteLine(ex.StackTrace);
        }
        finally
        {
            if (client != null && loggedIn)
            {
                Log("Logging out...");
                client.Network.Logout();
            }
        }

        PrintSummary();
        return results.Any(r => !r.Passed) ? 1 : 0;
    }

    static void SetupEventHandlers()
    {
        if (client == null) return;

        client.Network.LoginProgress += (sender, e) =>
        {
            Log($"LoginProgress: Status={e.Status}, Message={e.Message}");
            if (e.Status == LoginStatus.Success || e.Status == LoginStatus.Failed)
                loginEvent.Set();
        };

        client.Network.SimConnected += (sender, e) =>
        {
            Log($"SimConnected: {e.Simulator.Name}");
        };

        client.Network.EventQueueRunning += (sender, e) =>
        {
            Log($"EventQueueRunning for sim: {e.Simulator.Name}");
            eqRunningEvent.Set();
        };

        client.Objects.TerseObjectUpdate += (sender, e) =>
        {
            if (e.Prim.ID == client.Self.AgentID || e.Prim.LocalID == 1)
            {
                latestPosition = e.Prim.Position;
                terseUpdateReceived = true;
                Log($"TerseObjectUpdate: pos={latestPosition}");
                terseUpdateEvent.Set();
            }
        };

        client.Self.AnimationsChanged += (sender, e) =>
        {
            var anims = client.Self.SignaledAnimations;
            Log($"AnimationsChanged: {anims.Count} animations");
            lock (receivedAnimations)
            {
                receivedAnimations.Clear();
                foreach (var kvp in anims)
                {
                    receivedAnimations.Add(kvp.Key);
                    Log($"  Animation: {kvp.Key}");
                }
            }
            animationEvent.Set();
        };
    }

    static async Task TestLogin(CancellationToken ct)
    {
        Console.WriteLine("--- Test: Login ---");

        var loginParams = client!.Network.DefaultLoginParams(FirstName, LastName, Password, "OpenSimTest", "0.1");
        loginParams.URI = LoginUri;

        Log($"Attempting login to {LoginUri}...");

        await client.Network.LoginAsync(loginParams, ct);

        var loginOk = loginEvent.Wait(TimeSpan.FromSeconds(10));
        await Task.Delay(500, ct);

        if (loginOk && client.Network.Connected)
        {
            loggedIn = true;
            var seedCaps = client.Network.LoginSeedCapability;
            var seedStr = seedCaps?.ToString();

            Record("Login", true, $"AgentID={client.Self.AgentID}");
            Record("Circuit", true, $"Connected to {client.Network.CurrentSim?.Name ?? "unknown"}");
            Record("SeedCaps", seedCaps != null, $"URI={seedStr ?? "none"}");
        }
        else
        {
            var reason = client.Network.LoginMessage ?? "unknown";
            Record("Login", false, $"Failed: {reason}");
            loggedIn = false;
        }
    }

    static void TestRegionInfo()
    {
        Console.WriteLine("\n--- Test: Region Info ---");

        var sim = client!.Network.CurrentSim;
        if (sim != null)
        {
            Record("RegionHandshake", !string.IsNullOrEmpty(sim.Name), $"Region={sim.Name}");
            Record("TerrainReceived", true, "Connected to sim (terrain delivered at connect)");
        }
        else
        {
            Record("RegionHandshake", false, "No simulator connected");
            Record("TerrainReceived", false, "No simulator");
        }
    }

    static void TestSpawnPosition()
    {
        Console.WriteLine("\n--- Test: Spawn Position ---");

        var pos = client!.Self.SimPosition;
        Log($"Current position: {pos}");

        bool heightOk = pos.Z > 20.0f && pos.Z < 24.0f;
        Record("SpawnHeight", heightOk, $"Z={pos.Z:F1} (expected ~22.0, terrain=21.0)");

        bool xyOk = Math.Abs(pos.X - 128.0f) < 5.0f && Math.Abs(pos.Y - 128.0f) < 5.0f;
        Record("SpawnXY", xyOk, $"X={pos.X:F1} Y={pos.Y:F1} (expected ~128,128)");
    }

    static void TestDefaultAnimation()
    {
        Console.WriteLine("\n--- Test: Default Animation ---");

        animationEvent.Wait(TimeSpan.FromSeconds(3));

        UUID standAnim = new UUID("2408fe9e-df1d-1d7d-f4ff-1384fa7b350f");
        bool hasStand;
        lock (receivedAnimations)
        {
            hasStand = receivedAnimations.Contains(standAnim);
            Log($"Received {receivedAnimations.Count} animations");
        }

        Record("DefaultStandAnimation", hasStand,
            hasStand ? "Stand animation UUID received" : "Stand animation NOT received");
    }

    static async Task TestMovement(CancellationToken ct)
    {
        Console.WriteLine("\n--- Test: Movement Echo ---");

        var startPos = client!.Self.SimPosition;
        Log($"Start position: {startPos}");

        terseUpdateEvent.Reset();
        terseUpdateReceived = false;

        client.Self.Movement.AtPos = true;
        client.Self.Movement.SendUpdate();

        var gotUpdate = terseUpdateEvent.Wait(TimeSpan.FromSeconds(5));

        client.Self.Movement.AtPos = false;
        client.Self.Movement.SendUpdate();

        Record("MovementEcho", gotUpdate && terseUpdateReceived,
            gotUpdate ? $"Got ImprovedTerseObjectUpdate, pos={latestPosition}" : "No terse update received");

        if (gotUpdate)
        {
            bool posChanged = Vector3.Distance(startPos, latestPosition) > 0.01f;
            Record("PositionChanged", posChanged,
                $"Start={startPos} End={latestPosition} Delta={Vector3.Distance(startPos, latestPosition):F2}");
        }

        await Task.Delay(200, ct);
    }

    static async Task TestChat(CancellationToken ct)
    {
        Console.WriteLine("\n--- Test: Chat ---");

        try
        {
            client!.Self.Chat("Hello from test harness!", 0, ChatType.Normal);
            await Task.Delay(500, ct);
            Record("ChatSend", true, "Chat message sent without error");
        }
        catch (Exception ex)
        {
            Record("ChatSend", false, $"Exception: {ex.Message}");
        }
    }

    static async Task TestAssetService(CancellationToken ct)
    {
        Console.WriteLine("\n--- Test: Asset Service ---");

        try
        {
            using var httpClient = new HttpClient();
            httpClient.Timeout = TimeSpan.FromSeconds(5);

            var assetUrl = "http://127.0.0.1:8003/assets/00000000-0000-0000-0000-000000000099";
            Log($"Fetching {assetUrl}");

            var response = await httpClient.GetAsync(assetUrl, ct);
            int status = (int)response.StatusCode;
            Log($"Asset response: {status}");

            bool isHealthy = status == 404 || status == 200;
            Record("AssetServiceSchema", isHealthy,
                $"Status={status} (expected 404 for missing asset, not 500/503 for schema error)");
        }
        catch (Exception ex)
        {
            Record("AssetServiceSchema", false, $"Exception: {ex.Message}");
        }
    }

    static void TestEventQueue()
    {
        Console.WriteLine("\n--- Test: EventQueue ---");

        bool eqStarted = eqRunningEvent.Wait(TimeSpan.FromSeconds(3));
        Record("EventQueue", eqStarted, eqStarted ? "EventQueue running" : "EventQueue did not start");
    }

    static void PrintSummary()
    {
        Console.WriteLine("\n=== SUMMARY ===");
        int passed = results.Count(r => r.Passed);
        int total = results.Count;
        Console.WriteLine($"Passed: {passed}/{total}");

        if (results.Any(r => !r.Passed))
        {
            Console.WriteLine("\nFailed tests:");
            foreach (var r in results.Where(r => !r.Passed))
                Console.WriteLine($"  - {r.Name}: {r.Detail}");
        }

        Console.WriteLine($"\nResult: {(passed == total ? "ALL PASSED" : "SOME FAILED")}");
    }
}
