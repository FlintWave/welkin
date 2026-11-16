// SPDX-License-Identifier: LGPL-3.0-or-later
using System.Globalization;
using System.Text;
using LibreMetaverse;

internal static class Program
{
    static async Task<int> Main(string[] args)
    {
        string uri = Environment.GetEnvironmentVariable("WELKIN_LOGIN_URI") ?? "http://127.0.0.1:8002/";
        string first = Environment.GetEnvironmentVariable("WELKIN_BOT_FIRST") ?? "Bot";
        string last = Environment.GetEnvironmentVariable("WELKIN_BOT_LAST") ?? "Alpha";
        string pw = Environment.GetEnvironmentVariable("WELKIN_BOT_PASSWORD") ?? "password";
        string cmdFile = Environment.GetEnvironmentVariable("WELKIN_CMD_FILE") ?? "puppet.cmd";

        var c = new GridClient();
        var pendingFriends = new Dictionary<UUID, UUID>();
        var avatarIds = new Dictionary<string, UUID>(StringComparer.OrdinalIgnoreCase);
        var startedAnims = new HashSet<UUID>();
        var pendLock = new object();
        var avLock = new object();
        var printLock = new object();
        void Log(string line) { lock (printLock) { Console.WriteLine(line); Console.Out.Flush(); } }

        CancellationTokenSource? moveCts = null;
        long fileOffset = 0;
        var connectedEvt = new ManualResetEventSlim(false);
        c.Network.SimConnected += (_, __) => connectedEvt.Set();

        c.Self.ChatFromSimulator += (_, e) =>
        {
            if (e.SourceType != ChatSourceType.Agent) return;
            if (string.IsNullOrWhiteSpace(e.Message)) return;
            if (e.FromName == c.Self.Name) return;
            lock (avLock) avatarIds[e.FromName.ToLowerInvariant()] = e.SourceID;
            Log($"CHAT {e.FromName}: {e.Message}");
        };
        c.Self.IM += (_, e) =>
        {
            var im = e.IM;
            if (im.Dialog == InstantMessageDialog.FriendshipOffered)
            {
                lock (pendLock) pendingFriends[im.FromAgentID] = im.IMSessionID;
                Log($"FRIENDREQ {im.FromAgentName} {im.FromAgentID}");
                return;
            }
            if (im.Dialog != InstantMessageDialog.MessageFromAgent) return;
            if (im.FromAgentID == c.Self.AgentID) return;
            if (string.IsNullOrWhiteSpace(im.Message)) return;
            lock (avLock) avatarIds[im.FromAgentName.ToLowerInvariant()] = im.FromAgentID;
            Log($"IM {im.FromAgentName} {im.FromAgentID}: {im.Message}");
        };
        c.Friends.FriendshipOffered += (_, e) =>
        {
            lock (pendLock) pendingFriends[e.AgentID] = e.SessionID;
            Log($"FRIENDREQ {e.AgentName} {e.AgentID}");
        };
        c.Objects.AvatarUpdate += (_, e) =>
        {
            if (!e.IsNew) return;
            lock (avLock) avatarIds[e.Avatar.Name.ToLowerInvariant()] = e.Avatar.ID;
            Log($"AVATAR {e.Avatar.Name} {e.Avatar.ID} {e.Avatar.Position}");
        };

        var loginParams = c.Network.DefaultLoginParams(first, last, pw, "WelkinPuppetBot", "1.0");
        loginParams.URI = uri;
        loginParams.Start = "last";
        bool ok = await c.Network.LoginAsync(loginParams);
        if (!ok) { Console.WriteLine(c.Network.LoginMessage); return 1; }
        if (!connectedEvt.Wait(TimeSpan.FromSeconds(15))) { Console.WriteLine(c.Network.LoginMessage); return 1; }

        File.WriteAllText(cmdFile, string.Empty);
        Log($"READY {c.Self.Name} {c.Self.SimPosition}");

        var globalCts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; try { c.Network.Logout(); } catch { } globalCts.Cancel(); };

        bool disconnected = false;
        var keepalive = Task.Run(async () =>
        {
            try
            {
                while (!globalCts.Token.IsCancellationRequested)
                {
                    await Task.Delay(2000, globalCts.Token);
                    if (!c.Network.Connected) { disconnected = true; globalCts.Cancel(); Log("DISCONNECTED"); return; }
                    var m = c.Self.Movement;
                    if (!(m.AtPos || m.AtNeg || m.LeftPos || m.LeftNeg || m.UpPos || m.UpNeg))
                        m.SendUpdate(true);
                }
            }
            catch (OperationCanceledException) { }
        }, globalCts.Token);

        CancellationTokenSource StartMove()
        {
            try { moveCts?.Cancel(); } catch { }
            var cts = new CancellationTokenSource();
            moveCts = cts;
            return cts;
        }

        Avatar? FindAvatar(string token)
        {
            if (UUID.TryParse(token, out var id))
            {
                foreach (var a in c.Network.CurrentSim.ObjectsAvatars.Values)
                    if (a.ID == id) return a;
                return null;
            }
            foreach (var a in c.Network.CurrentSim.ObjectsAvatars.Values)
                if (a.Name.Equals(token, StringComparison.OrdinalIgnoreCase)) return a;
            return null;
        }

        void ProcessFile()
        {
            if (!File.Exists(cmdFile)) return;
            using var fs = new FileStream(cmdFile, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            if (fs.Length < fileOffset) fileOffset = 0;
            if (fs.Length <= fileOffset) return;
            fs.Seek(fileOffset, SeekOrigin.Begin);
            using var sr = new StreamReader(fs, Encoding.UTF8, true, 1024, true);
            string chunk = sr.ReadToEnd();
            int lastNl = chunk.LastIndexOf('\n');
            if (lastNl < 0) return;
            string complete = chunk.Substring(0, lastNl + 1);
            using var lr = new StringReader(complete);
            string? line;
            while ((line = lr.ReadLine()) != null) HandleLine(line);
            fileOffset += Encoding.UTF8.GetByteCount(complete);
        }

        void HandleLine(string raw)
        {
            string line = raw.Trim();
            if (line.Length == 0) return;
            int sp = line.IndexOf(' ');
            string verb = sp < 0 ? line : line.Substring(0, sp);
            string rest = sp < 0 ? string.Empty : line.Substring(sp + 1).TrimStart();
            string v = verb;
            switch (verb.ToLowerInvariant())
            {
                case "say":
                    c.Self.Chat(rest, 0, ChatType.Normal);
                    Log($"OK {v}");
                    break;
                case "shout":
                    c.Self.Chat(rest, 0, ChatType.Shout);
                    Log($"OK {v}");
                    break;
                case "whisper":
                    c.Self.Chat(rest, 0, ChatType.Whisper);
                    Log($"OK {v}");
                    break;
                case "im":
                {
                    string[] t = rest.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                    UUID id;
                    string text;
                    if (t.Length == 0) { Log($"ERR {v} bad args"); break; }
                    if (UUID.TryParse(t[0], out id))
                    {
                        if (t.Length < 2) { Log($"ERR {v} bad args"); break; }
                        text = string.Join(" ", t, 1, t.Length - 1);
                    }
                    else
                    {
                        if (t.Length < 3) { Log($"ERR {v} bad args"); break; }
                        string name = t[0] + " " + t[1];
                        lock (avLock)
                        {
                            if (!avatarIds.TryGetValue(name.ToLowerInvariant(), out id))
                            { Log($"ERR {v} unknown agent"); return; }
                        }
                        text = string.Join(" ", t, 2, t.Length - 2);
                    }
                    c.Self.InstantMessage(id, text);
                    Log($"OK {v}");
                    break;
                }
                case "goto":
                {
                    string[] t = rest.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                    if (t.Length < 2 ||
                        !float.TryParse(t[0], NumberStyles.Float, CultureInfo.InvariantCulture, out float tx) ||
                        !float.TryParse(t[1], NumberStyles.Float, CultureInfo.InvariantCulture, out float ty))
                    { Log($"ERR {v} bad args"); break; }
                    float tz = c.Self.SimPosition.Z;
                    if (t.Length >= 3 && float.TryParse(t[2], NumberStyles.Float, CultureInfo.InvariantCulture, out float tz2)) tz = tz2;
                    var target = new Vector3(tx, ty, tz);
                    var cur = c.Self.SimPosition;
                    double angle = Math.Atan2(target.Y - cur.Y, target.X - cur.X);
                    c.Self.Movement.BodyRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, (float)angle);
                    c.Self.Movement.HeadRotation = c.Self.Movement.BodyRotation;
                    c.Self.Movement.AtPos = true;
                    c.Self.Movement.SendUpdate(true);
                    var cts = StartMove();
                    var token = cts.Token;
                    _ = Task.Run(async () =>
                    {
                        bool arrived = false;
                        try
                        {
                            var start = DateTime.UtcNow;
                            while (!token.IsCancellationRequested)
                            {
                                var pcur = c.Self.SimPosition;
                                float ddx = target.X - pcur.X;
                                float ddy = target.Y - pcur.Y;
                                float dist = MathF.Sqrt(ddx * ddx + ddy * ddy);
                                if (dist < 1.5f) { arrived = true; break; }
                                if ((DateTime.UtcNow - start).TotalSeconds >= 20) { arrived = true; break; }
                                await Task.Delay(200, token);
                                c.Self.Movement.SendUpdate(true);
                            }
                        }
                        catch (OperationCanceledException) { arrived = false; }
                        finally
                        {
                            if (ReferenceEquals(moveCts, cts))
                            {
                                c.Self.Movement.AtPos = false;
                                c.Self.Movement.SendUpdate(true);
                                if (arrived) Log($"ARRIVED {c.Self.SimPosition}");
                            }
                        }
                    }, token);
                    Log($"OK {v}");
                    break;
                }
                case "follow":
                {
                    if (string.IsNullOrWhiteSpace(rest)) { Log($"ERR {v} bad args"); break; }
                    if (FindAvatar(rest) == null) { Log($"ERR {v} unknown avatar"); break; }
                    var cts = StartMove();
                    var token = cts.Token;
                    _ = Task.Run(async () =>
                    {
                        bool arrived = false;
                        try
                        {
                            while (!token.IsCancellationRequested)
                            {
                                var av = FindAvatar(rest);
                                if (av == null) break;
                                var tpos = av.Position;
                                var ccur = c.Self.SimPosition;
                                float ddx = tpos.X - ccur.X;
                                float ddy = tpos.Y - ccur.Y;
                                float dist = MathF.Sqrt(ddx * ddx + ddy * ddy);
                                if (dist <= 3f) { arrived = true; break; }
                                double angle = Math.Atan2(ddy, ddx);
                                c.Self.Movement.BodyRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, (float)angle);
                                c.Self.Movement.HeadRotation = c.Self.Movement.BodyRotation;
                                c.Self.Movement.AtPos = true;
                                c.Self.Movement.SendUpdate(true);
                                await Task.Delay(500, token);
                            }
                        }
                        catch (OperationCanceledException) { arrived = false; }
                        finally
                        {
                            if (ReferenceEquals(moveCts, cts))
                            {
                                c.Self.Movement.AtPos = false;
                                c.Self.Movement.SendUpdate(true);
                                if (arrived) Log($"ARRIVED {c.Self.SimPosition}");
                            }
                        }
                    }, token);
                    Log($"OK {v}");
                    break;
                }
                case "turn":
                {
                    if (!float.TryParse(rest, NumberStyles.Float, CultureInfo.InvariantCulture, out float deg))
                    { Log($"ERR {v} bad args"); break; }
                    c.Self.Movement.BodyRotation = c.Self.Movement.BodyRotation * Quaternion.CreateFromAxisAngle(Vector3.UnitZ, (float)(deg * Math.PI / 180.0));
                    c.Self.Movement.HeadRotation = c.Self.Movement.BodyRotation;
                    c.Self.Movement.SendUpdate(true);
                    Log($"OK {v}");
                    break;
                }
                case "face":
                {
                    string[] t = rest.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                    if (t.Length < 2 ||
                        !float.TryParse(t[0], NumberStyles.Float, CultureInfo.InvariantCulture, out float fx) ||
                        !float.TryParse(t[1], NumberStyles.Float, CultureInfo.InvariantCulture, out float fy))
                    { Log($"ERR {v} bad args"); break; }
                    var pcur = c.Self.SimPosition;
                    double angle = Math.Atan2(fy - pcur.Y, fx - pcur.X);
                    c.Self.Movement.BodyRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, (float)angle);
                    c.Self.Movement.HeadRotation = c.Self.Movement.BodyRotation;
                    c.Self.Movement.SendUpdate(true);
                    Log($"OK {v}");
                    break;
                }
                case "walk":
                case "back":
                {
                    if (!double.TryParse(rest, NumberStyles.Float, CultureInfo.InvariantCulture, out double sec) || sec < 0)
                    { Log($"ERR {v} bad args"); break; }
                    var cts = StartMove();
                    var token = cts.Token;
                    bool forward = verb.Equals("walk", StringComparison.OrdinalIgnoreCase);
                    if (forward) c.Self.Movement.AtPos = true;
                    else c.Self.Movement.AtNeg = true;
                    c.Self.Movement.SendUpdate(true);
                    _ = Task.Run(async () =>
                    {
                        bool done = false;
                        try
                        {
                            await Task.Delay(TimeSpan.FromSeconds(sec), token);
                            done = true;
                        }
                        catch (OperationCanceledException) { }
                        finally
                        {
                            if (done && ReferenceEquals(moveCts, cts))
                            {
                                if (forward) c.Self.Movement.AtPos = false;
                                else c.Self.Movement.AtNeg = false;
                                c.Self.Movement.SendUpdate(true);
                            }
                        }
                    }, token);
                    Log($"OK {v}");
                    break;
                }
                case "stop":
                    moveCts?.Cancel();
                    moveCts = null;
                    var mm = c.Self.Movement;
                    mm.AtPos = false; mm.AtNeg = false; mm.LeftPos = false; mm.LeftNeg = false; mm.UpPos = false; mm.UpNeg = false;
                    mm.SendUpdate(true);
                    Log($"OK {v}");
                    break;
                case "sit":
                    c.Self.SitOnGround();
                    Log($"OK {v}");
                    break;
                case "stand":
                    c.Self.Stand();
                    Log($"OK {v}");
                    break;
                case "fly":
                {
                    string t = rest.Trim().ToLowerInvariant();
                    if (t != "on" && t != "off") { Log($"ERR {v} bad args"); break; }
                    c.Self.Movement.Fly = t == "on";
                    c.Self.Movement.SendUpdate(true);
                    Log($"OK {v}");
                    break;
                }
                case "up":
                {
                    if (!double.TryParse(rest, NumberStyles.Float, CultureInfo.InvariantCulture, out double sec) || sec < 0)
                    { Log($"ERR {v} bad args"); break; }
                    c.Self.Movement.UpPos = true;
                    c.Self.Movement.SendUpdate(true);
                    _ = Task.Run(async () =>
                    {
                        try { await Task.Delay(TimeSpan.FromSeconds(sec)); }
                        catch { }
                        c.Self.Movement.UpPos = false;
                        c.Self.Movement.SendUpdate(true);
                    });
                    Log($"OK {v}");
                    break;
                }
                case "jump":
                    c.Self.Movement.UpPos = true;
                    c.Self.Movement.SendUpdate(true);
                    _ = Task.Run(async () =>
                    {
                        await Task.Delay(200);
                        c.Self.Movement.UpPos = false;
                        c.Self.Movement.SendUpdate(true);
                    });
                    Log($"OK {v}");
                    break;
                case "anim":
                {
                    if (rest.Equals("stop", StringComparison.OrdinalIgnoreCase))
                    {
                        foreach (var id in startedAnims) c.Self.AnimationStop(id, true);
                        startedAnims.Clear();
                        Log($"OK {v}");
                        break;
                    }
                    var animMap = new Dictionary<string, UUID>(StringComparer.OrdinalIgnoreCase)
                    {
                        ["wave"] = Animations.HELLO,
                        ["dance"] = Animations.DANCE1,
                        ["dance2"] = Animations.DANCE2,
                        ["clap"] = Animations.CLAP,
                        ["bow"] = Animations.BOW,
                        ["laugh"] = Animations.LAUGH_SHORT,
                        ["cry"] = Animations.CRY,
                        ["shrug"] = Animations.SHRUG,
                        ["point"] = Animations.POINT_YOU,
                        ["kiss"] = Animations.BLOW_KISS,
                    };
                    if (!animMap.TryGetValue(rest.Trim(), out var animId))
                    { Log($"ERR {v} unknown anim"); break; }
                    c.Self.AnimationStart(animId, true);
                    startedAnims.Add(animId);
                    Log($"OK {v}");
                    break;
                }
                case "accept":
                case "decline":
                {
                    bool accept = verb.Equals("accept", StringComparison.OrdinalIgnoreCase);
                    string[] t = rest.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                    if (t.Length == 0) { Log($"ERR {v} bad args"); break; }
                    UUID agentId;
                    if (UUID.TryParse(t[0], out agentId)) { }
                    else
                    {
                        if (t.Length < 2) { Log($"ERR {v} bad args"); break; }
                        string name = t[0] + " " + t[1];
                        lock (avLock)
                        {
                            if (!avatarIds.TryGetValue(name.ToLowerInvariant(), out agentId))
                            { Log($"ERR {v} unknown agent"); break; }
                        }
                    }
                    lock (pendLock)
                    {
                        if (!pendingFriends.TryGetValue(agentId, out var sessionId))
                        { Log($"ERR {v} no pending request"); break; }
                        if (accept) c.Friends.AcceptFriendship(agentId, sessionId);
                        else c.Friends.DeclineFriendship(agentId, sessionId);
                        pendingFriends.Remove(agentId);
                    }
                    Log($"OK {v}");
                    break;
                }
                case "where":
                {
                    var avs = c.Network.CurrentSim.ObjectsAvatars.Values.ToList();
                    Log($"POS {c.Self.SimPosition} AVATARS {avs.Count}");
                    foreach (var a in avs) Log($"AVATAR {a.Name} {a.ID} {a.Position}");
                    Log($"OK {v}");
                    break;
                }
                case "quit":
                    Log($"OK {v}");
                    try { c.Network.Logout(); } catch { }
                    System.Environment.Exit(0);
                    break;
                default:
                    Log($"ERR {v} unknown command");
                    break;
            }
        }

        int exitCode = 0;
        bool running = true;
        while (running && !globalCts.IsCancellationRequested)
        {
            try { ProcessFile(); }
            catch (Exception ex) { Log($"ERR IO {ex.Message}"); }
            if (disconnected) { exitCode = 2; break; }
            try { await Task.Delay(200, globalCts.Token); }
            catch (OperationCanceledException) { break; }
        }
        if (disconnected) exitCode = 2;
        globalCts.Cancel();
        try { await keepalive; } catch { }
        try { c.Network.Logout(); } catch { }
        return exitCode;
    }
}
