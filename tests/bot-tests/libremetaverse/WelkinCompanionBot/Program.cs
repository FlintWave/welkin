// SPDX-License-Identifier: LGPL-3.0-or-later
using LibreMetaverse;

return await Run();

static async Task<int> Run()
{
    var uri = Environment.GetEnvironmentVariable("WELKIN_LOGIN_URI") ?? "http://127.0.0.1:8002/";
    var first = Environment.GetEnvironmentVariable("WELKIN_BOT_FIRST") ?? "Bot";
    var last = Environment.GetEnvironmentVariable("WELKIN_BOT_LAST") ?? "Alpha";
    var pw = Environment.GetEnvironmentVariable("WELKIN_BOT_PASSWORD") ?? "password";

    var c = new GridClient();
    using var simConnected = new ManualResetEventSlim(false);
    c.Network.SimConnected += (_, __) => simConnected.Set();

    var p = c.Network.DefaultLoginParams(first, last, pw, "WelkinCompanionBot", "1.0");
    p.URI = uri;
    p.Start = "last";

    bool ok = await c.Network.LoginAsync(p);
    if (!ok)
    {
        Console.WriteLine(c.Network.LoginMessage);
        return 1;
    }

    if (!simConnected.Wait(15000))
    {
        Console.WriteLine("Timed out waiting for sim connection");
        try { c.Network.Logout(); } catch { }
        return 1;
    }

    Console.WriteLine($"Companion bot online as {first} {last} at {c.Self.SimPosition}");

    var cts = new CancellationTokenSource();
    bool disconnected = false;
    bool userCancelled = false;
    DateTime lastReply = DateTime.MinValue;
    var replyLock = new object();
    var acceptedSessions = new HashSet<UUID>();
    var rnd = new Random();
    bool walking = false;

    Console.CancelKeyPress += (_, e) => { e.Cancel = true; userCancelled = true; cts.Cancel(); };

    c.Self.ChatFromSimulator += (s, e) =>
    {
        if (e.SourceType != ChatSourceType.Agent) return;
        if (e.FromName == c.Self.Name) return;
        if (string.IsNullOrWhiteSpace(e.Message)) return;

        Console.WriteLine($"[CHAT RX] {e.FromName}: {e.Message}");

        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(700, cts.Token);

                string reply;
                if (ContainsWord(e.Message, "hello") || ContainsWord(e.Message, "hi"))
                    reply = $"Hello {e.FromName}! Nice to meet you.";
                else if (e.Message.Contains("fly", StringComparison.OrdinalIgnoreCase))
                    reply = "I'm staying on the ground, but go ahead and fly!";
                else if (e.Message.Contains("how are you", StringComparison.OrdinalIgnoreCase))
                    reply = "Doing well, thanks for asking.";
                else
                    reply = $"You said: {e.Message}";

                bool send;
                lock (replyLock)
                {
                    send = (DateTime.UtcNow - lastReply).TotalSeconds >= 2;
                    if (send) lastReply = DateTime.UtcNow;
                }

                if (!send) return;

                c.Self.Chat(reply, 0, ChatType.Normal);
                Console.WriteLine($"[CHAT TX] Bot: {reply}");
            }
            catch (OperationCanceledException) { }
        }, cts.Token);
    };

    c.Self.IM += (s, e) =>
    {
        var im = e.IM;
        if (im.Dialog != InstantMessageDialog.MessageFromAgent && im.Dialog != InstantMessageDialog.FriendshipOffered)
            return;

        if (im.Dialog == InstantMessageDialog.FriendshipOffered)
        {
            if (acceptedSessions.Add(im.IMSessionID))
            {
                c.Friends.AcceptFriendship(im.FromAgentID, im.IMSessionID);
                Console.WriteLine($"[FRIEND] Accepted friendship from {im.FromAgentName}");
            }
            return;
        }

        if (im.FromAgentID == c.Self.AgentID) return;
        if (string.IsNullOrWhiteSpace(im.Message)) return;

        Console.WriteLine($"[IM RX] {im.FromAgentName}: {im.Message}");
        string reply = $"Hi {im.FromAgentName}, I got your message: {im.Message}";
        c.Self.InstantMessage(im.FromAgentID, reply);
        Console.WriteLine($"[IM TX] Bot: {reply}");
    };

    c.Friends.FriendshipOffered += (s, e) =>
    {
        c.Friends.AcceptFriendship(e.AgentID, e.SessionID);
        Console.WriteLine($"[FRIEND] Accepted friendship from {e.AgentID}");
    };

    _ = Task.Run(async () =>
    {
        while (!cts.IsCancellationRequested)
        {
            if (!c.Network.Connected)
            {
                Console.WriteLine("Disconnected");
                disconnected = true;
                cts.Cancel();
                return;
            }
            try { await Task.Delay(1000, cts.Token); }
            catch (OperationCanceledException) { return; }
        }
    }, cts.Token);

    async Task WalkAsync()
    {
        if (walking) return;
        walking = true;
        try
        {
            if ((DateTime.UtcNow - lastReply).TotalSeconds < 5) return;

            var pos = c.Self.SimPosition;
            float tx = Math.Clamp((float)(pos.X + rnd.NextDouble() * 24 - 12), 20f, 236f);
            float ty = Math.Clamp((float)(pos.Y + rnd.NextDouble() * 24 - 12), 20f, 236f);
            var target = new Vector3(tx, ty, pos.Z);

            double angle = Math.Atan2(target.Y - pos.Y, target.X - pos.X);
            c.Self.Movement.BodyRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, (float)angle);
            c.Self.Movement.HeadRotation = c.Self.Movement.BodyRotation;
            c.Self.Movement.AtPos = true;
            c.Self.Movement.SendUpdate(true);

            var start = DateTime.UtcNow;
            while (!cts.IsCancellationRequested)
            {
                var pp = c.Self.SimPosition;
                float dx = pp.X - target.X;
                float dy = pp.Y - target.Y;
                float dz = pp.Z - target.Z;
                if (MathF.Sqrt(dx * dx + dy * dy + dz * dz) <= 1.5f) break;
                if ((DateTime.UtcNow - start).TotalSeconds >= 12) break;
                await Task.Delay(200, cts.Token);
                c.Self.Movement.SendUpdate(true);
            }

            c.Self.Movement.AtPos = false;
            c.Self.Movement.SendUpdate(true);
            Console.WriteLine($"[WALK] Target {target}");
        }
        catch (OperationCanceledException) { }
        finally { walking = false; }
    }

    _ = Task.Run(async () =>
    {
        try
        {
            await Task.Delay(10000, cts.Token);
            while (!cts.IsCancellationRequested)
            {
                if ((DateTime.UtcNow - lastReply).TotalSeconds >= 5)
                    await WalkAsync();
                await Task.Delay(30000, cts.Token);
            }
        }
        catch (OperationCanceledException) { }
    }, cts.Token);

    _ = Task.Run(async () =>
    {
        while (!cts.IsCancellationRequested)
        {
            if (!walking) c.Self.Movement.SendUpdate(true);
            try { await Task.Delay(2000, cts.Token); }
            catch (OperationCanceledException) { return; }
        }
    }, cts.Token);

    try { await Task.Delay(Timeout.Infinite, cts.Token); }
    catch (OperationCanceledException) { }

    try { c.Network.Logout(); } catch { }
    Console.WriteLine("Companion bot offline.");
    return disconnected ? 2 : 0;
}

static bool ContainsWord(string text, string word)
{
    var lower = text.ToLowerInvariant();
    int i = 0;
    while ((i = lower.IndexOf(word, i, StringComparison.Ordinal)) >= 0)
    {
        bool left = i == 0 || !char.IsLetterOrDigit(lower[i - 1]);
        int end = i + word.Length;
        bool right = end == lower.Length || !char.IsLetterOrDigit(lower[end]);
        if (left && right) return true;
        i = end > i ? end : i + 1;
    }
    return false;
}
