// SPDX-License-Identifier: LGPL-3.0-or-later
using System;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using LibreMetaverse;

namespace WelkinBotTest;

class Program
{
    static async Task<int> Main()
    {
        bool loginOk = false, regionOk = false, chatSentOk = false, chatRecvOk = false;
        bool moveOk = false, rezOk = false, logoutOk = false;
        bool scriptUploadOk = false, scriptAliveOk = false, touchOk = false;
        string regionName = "";

        using var client = new GridClient();

        var regionReady = new ManualResetEventSlim(false);
        var chatReceived = new ManualResetEventSlim(false);
        var primRezzed = new ManualResetEventSlim(false);
        var scriptAlive = new ManualResetEventSlim(false);
        var touchResponse = new ManualResetEventSlim(false);
        uint rezzedLocalId = 0;
        UUID rezzedPrimUuid = UUID.Zero;

        client.Network.SimConnected += (_, e) =>
        {
            regionName = e.Simulator.Name;
            Console.WriteLine($"[REGION] Connected to: {regionName}");
            regionOk = true;
            regionReady.Set();
        };

        client.Objects.ObjectUpdate += (_, e) =>
        {
            if (e.IsNew && e.Prim.PrimData.PCode == PCode.Prim && e.Prim.OwnerID == client.Self.AgentID)
            {
                rezzedLocalId = e.Prim.LocalID;
                rezzedPrimUuid = e.Prim.ID;
                Console.WriteLine($"[REZ] New prim created: local_id={rezzedLocalId}, uuid={rezzedPrimUuid}");
                primRezzed.Set();
            }
        };

        client.Self.ChatFromSimulator += (_, e) =>
        {
            Console.WriteLine($"[CHAT] {e.FromName}: {e.Message} (type={e.Type}, source={e.SourceType})");
            if (e.Message.Contains("Bot test: hello from LibreMetaverse"))
            {
                chatRecvOk = true;
                chatReceived.Set();
            }
            if (e.Message.Contains("SCRIPT_ALIVE"))
            {
                scriptAliveOk = true;
                scriptAlive.Set();
            }
            if (e.Message.Contains("TOUCHED"))
            {
                touchOk = true;
                touchResponse.Set();
            }
        };

        Console.WriteLine("[LOGIN] Logging in as Test User...");
        var loginParams = client.Network.DefaultLoginParams("Test", "User", "password", "WelkinBotTest", "1.0");
        loginParams.URI = Environment.GetEnvironmentVariable("WELKIN_LOGIN_URI") ?? "http://127.0.0.1:8002/";
        loginParams.Start = "last";

        bool success = await client.Network.LoginAsync(loginParams);

        if (success)
        {
            loginOk = true;
            Console.WriteLine($"[LOGIN] OK — agent={client.Self.AgentID}, session={client.Self.SessionID}");
        }
        else
        {
            Console.WriteLine($"[LOGIN] FAILED — {client.Network.LoginMessage}");
            PrintSummary(loginOk, regionOk, chatSentOk, chatRecvOk, moveOk, rezOk, logoutOk, scriptUploadOk, scriptAliveOk, touchOk);
            return 1;
        }

        if (!regionReady.Wait(TimeSpan.FromSeconds(15)))
        {
            Console.WriteLine("[REGION] Timeout waiting for SimConnected");
        }

        await Task.Delay(1000);

        Console.WriteLine("[CHAT] Sending: Bot test: hello from LibreMetaverse");
        client.Self.Chat("Bot test: hello from LibreMetaverse", 0, ChatType.Normal);
        chatSentOk = true;

        chatReceived.Wait(TimeSpan.FromSeconds(5));

        var posBefore = client.Self.SimPosition;
        Console.WriteLine($"[MOVE] Position before: {posBefore}");

        client.Self.Movement.BodyRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, 0f);
        client.Self.Movement.HeadRotation = client.Self.Movement.BodyRotation;
        client.Self.Movement.AtPos = true;
        client.Self.Movement.SendUpdate(true);

        for (int i = 0; i < 10; i++)
        {
            await Task.Delay(500);
            client.Self.Movement.SendUpdate(true);
        }

        client.Self.Movement.AtPos = false;
        client.Self.Movement.SendUpdate(true);
        await Task.Delay(1000);

        var posAfter = client.Self.SimPosition;
        Console.WriteLine($"[MOVE] Position after:  {posAfter}");

        float dist = (posAfter - posBefore).Length();
        Console.WriteLine($"[MOVE] Distance moved: {dist:F2}");
        moveOk = dist > 0.1f;

        Console.WriteLine("[REZ] Creating a box prim...");
        var primData = ObjectManager.BuildBasicShape(PrimType.Box);
        var rezPos = client.Self.SimPosition + new Vector3(2f, 0f, 0f);
        client.Objects.AddPrim(client.Network.CurrentSim, primData, UUID.Zero,
            rezPos, new Vector3(0.5f, 0.5f, 0.5f), Quaternion.Identity);

        if (primRezzed.Wait(TimeSpan.FromSeconds(5)))
        {
            rezOk = true;
            Console.WriteLine($"[REZ] PASS — prim created at local_id={rezzedLocalId}");
        }
        else
        {
            Console.WriteLine("[REZ] Timeout waiting for prim creation confirmation");
        }

        if (rezOk)
        {
            Console.WriteLine($"[SCRIPT] Uploading script to prim {rezzedPrimUuid}...");
            try
            {
                using var http = new HttpClient();
                var lslCode = "default { state_entry() { llSay(0, \"SCRIPT_ALIVE\"); } touch_start(integer n) { llSay(0, \"TOUCHED\"); } }";
                var content = new StringContent(lslCode);
                content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("text/plain");
                var response = await http.PostAsync($"{Environment.GetEnvironmentVariable("WELKIN_REGION_HTTP") ?? "http://127.0.0.1:9001"}/UpdateScriptAgent?prim_id={rezzedPrimUuid}", content);
                if (response.IsSuccessStatusCode)
                {
                    scriptUploadOk = true;
                    Console.WriteLine($"[SCRIPT] Uploaded script to prim {rezzedPrimUuid}");
                }
                else
                {
                    Console.WriteLine($"[SCRIPT] Upload failed: HTTP {(int)response.StatusCode}");
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"[SCRIPT] Upload error: {ex.Message}");
            }
        }

        if (scriptUploadOk)
        {
            scriptAlive.Wait(TimeSpan.FromSeconds(10));
            Console.WriteLine($"[SCRIPT] Script alive: {(scriptAliveOk ? "PASS" : "FAIL")}");
        }
        else
        {
            Console.WriteLine("[SCRIPT] Script alive: SKIP (upload not ok)");
        }

        if (scriptAliveOk)
        {
            await client.Objects.ClickObjectAsync(client.Network.CurrentSim, rezzedLocalId);
            Console.WriteLine($"[TOUCH] Sent ObjectGrab to local_id={rezzedLocalId}");
            touchResponse.Wait(TimeSpan.FromSeconds(10));
            Console.WriteLine($"[TOUCH] Touch event: {(touchOk ? "PASS" : "FAIL")}");
        }
        else
        {
            Console.WriteLine("[TOUCH] Touch event: SKIP (script not alive)");
        }

        Console.WriteLine("[LOGOUT] Logging out...");
        client.Network.Logout();
        logoutOk = true;
        Console.WriteLine("[LOGOUT] Done");

        PrintSummary(loginOk, regionOk, chatSentOk, chatRecvOk, moveOk, rezOk, logoutOk, scriptUploadOk, scriptAliveOk, touchOk);
        return (loginOk && regionOk && chatSentOk && logoutOk) ? 0 : 1;
    }

    static void PrintSummary(bool login, bool region, bool chatSent, bool chatRecv, bool move, bool rez, bool logout, bool scriptUpload, bool scriptAlive, bool touch)
    {
        Console.WriteLine();
        Console.WriteLine("=== Bot Test Summary ===");
        Console.WriteLine($"  Login:          {(login ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Region connect: {(region ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Chat sent:      {(chatSent ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Chat echo:      {(chatRecv ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Movement:       {(move ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Rez prim:       {(rez ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Script upload:  {(scriptUpload ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Script alive:   {(scriptAlive ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Touch event:    {(touch ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Logout:         {(logout ? "PASS" : "FAIL")}");
    }
}
