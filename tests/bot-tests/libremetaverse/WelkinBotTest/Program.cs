using System;
using System.Threading;
using System.Threading.Tasks;
using LibreMetaverse;

namespace WelkinBotTest;

class Program
{
    static async Task<int> Main()
    {
        bool loginOk = false, regionOk = false, chatSentOk = false, chatRecvOk = false;
        bool moveOk = false, logoutOk = false;
        string regionName = "";

        using var client = new GridClient();

        var regionReady = new ManualResetEventSlim(false);
        var chatReceived = new ManualResetEventSlim(false);

        client.Network.SimConnected += (_, e) =>
        {
            regionName = e.Simulator.Name;
            Console.WriteLine($"[REGION] Connected to: {regionName}");
            regionOk = true;
            regionReady.Set();
        };

        client.Self.ChatFromSimulator += (_, e) =>
        {
            Console.WriteLine($"[CHAT] {e.FromName}: {e.Message} (type={e.Type}, source={e.SourceType})");
            if (e.Message.Contains("Bot test: hello from LibreMetaverse"))
            {
                chatRecvOk = true;
                chatReceived.Set();
            }
        };

        Console.WriteLine("[LOGIN] Logging in as Test User...");
        var loginParams = client.Network.DefaultLoginParams("Test", "User", "password", "WelkinBotTest", "1.0");
        loginParams.URI = "http://127.0.0.1:8002/";
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
            PrintSummary(loginOk, regionOk, chatSentOk, chatRecvOk, moveOk, logoutOk);
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

        Console.WriteLine("[LOGOUT] Logging out...");
        client.Network.Logout();
        logoutOk = true;
        Console.WriteLine("[LOGOUT] Done");

        PrintSummary(loginOk, regionOk, chatSentOk, chatRecvOk, moveOk, logoutOk);
        return (loginOk && regionOk && chatSentOk && logoutOk) ? 0 : 1;
    }

    static void PrintSummary(bool login, bool region, bool chatSent, bool chatRecv, bool move, bool logout)
    {
        Console.WriteLine();
        Console.WriteLine("=== Bot Test Summary ===");
        Console.WriteLine($"  Login:          {(login ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Region connect: {(region ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Chat sent:      {(chatSent ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Chat echo:      {(chatRecv ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Movement:       {(move ? "PASS" : "FAIL")}");
        Console.WriteLine($"  Logout:         {(logout ? "PASS" : "FAIL")}");
    }
}
