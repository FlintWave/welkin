// SPDX-License-Identifier: LGPL-3.0-or-later
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using LibreMetaverse;

class Bot { public GridClient Client; public string Name; }
class Outcome { public string Name; public string Status; public string Reason; }

class Program
{
    static string LoginUri = Environment.GetEnvironmentVariable("WELKIN_LOGIN_URI") ?? "http://127.0.0.1:8102/";
    static List<Outcome> Outcomes = new();
    static void Record(string name, string status, string reason)
    {
        Console.WriteLine($"{name}: {status} {reason}");
        Outcomes.Add(new Outcome { Name = name, Status = status, Reason = reason });
    }
    static async Task<bool> WaitFor(Func<bool> cond, int ms)
    {
        int elapsed = 0;
        while (elapsed < ms) { if (cond()) return true; await Task.Delay(100); elapsed += 100; }
        return cond();
    }
    static async Task<bool> LoginBot(Bot bot, string first, string last)
    {
        bot.Client = new GridClient();
        var p = bot.Client.Network.DefaultLoginParams(first, last, "password", "WelkinMultiBotTest", "1.0");
        p.URI = LoginUri;
        p.Start = "last";
        var ev = new ManualResetEventSlim(false);
        bot.Client.Network.SimConnected += (s, e) => ev.Set();
        bool ok = false;
        try { ok = await bot.Client.Network.LoginAsync(p); } catch (Exception ex) { Console.WriteLine($"Login exception {bot.Name}: {ex.Message}"); }
        if (ok) ev.Wait(15000);
        await Task.Delay(3000);
        return ok && ev.IsSet;
    }
    static async Task<int> Main()
    {
        var alpha = new Bot { Name = "Bot Alpha" };
        var bravo = new Bot { Name = "Bot Bravo" };
        try
        {
            bool aOk = await LoginBot(alpha, "Bot", "Alpha");
            bool bOk = await LoginBot(bravo, "Bot", "Bravo");
            if (!aOk || !bOk)
            {
                foreach (var t in new[] { "ChatBotToBot", "Proximity", "InstantMessage", "ObjectHandoff", "GroupLifecycle", "HumanCoexistence" })
                    Record(t, "SKIP", "Login failed");
                return 1;
            }
            UUID alphaId = alpha.Client.Self.AgentID;
            UUID bravoId = bravo.Client.Self.AgentID;

            if (Environment.GetEnvironmentVariable("WELKIN_WAIT_FOR_HUMAN") == "1")
            {
                Console.WriteLine("Waiting for human viewer...");
                DateTime start = DateTime.UtcNow; bool found = false;
                while ((DateTime.UtcNow - start).TotalSeconds < 120 && !found)
                {
                    found = alpha.Client.Network.CurrentSim.ObjectsAvatars.Values.Any(a => !a.Name.Contains("Bot "));
                    if (!found) await Task.Delay(500);
                }
                Console.WriteLine(found ? "Human viewer detected." : "Proceeding without human viewer.");
            }

            // 1 ChatBotToBot
            {
                string msg = "alpha-chat-" + Guid.NewGuid();
                bool rx = false; string from = null;
                bravo.Client.Self.ChatFromSimulator += (s, e) =>
                {
                    if (!rx && e.Message == msg && e.FromName != null && e.FromName.Contains("Alpha") && e.SourceType == ChatSourceType.Agent)
                    { rx = true; from = e.FromName; }
                };
                alpha.Client.Self.Chat(msg, 0, ChatType.Normal);
                bool ok = await WaitFor(() => rx, 5000);
                Record("ChatBotToBot", ok ? "PASS" : "FAIL", ok ? $"Heard \"{msg}\" from {from}" : "No matching chat received");
            }

            // 2 Proximity
            {
                bool bSawA = false, aSawB = false;
                alpha.Client.Objects.AvatarUpdate += (s, e) => { if (e.Avatar.ID == bravoId) aSawB = true; };
                bravo.Client.Objects.AvatarUpdate += (s, e) => { if (e.Avatar.ID == alphaId) bSawA = true; };
                bool ok = await WaitFor(() =>
                {
                    if (!bSawA) foreach (var a in bravo.Client.Network.CurrentSim.ObjectsAvatars.Values) if (a.ID == alphaId) bSawA = true;
                    if (!aSawB) foreach (var a in alpha.Client.Network.CurrentSim.ObjectsAvatars.Values) if (a.ID == bravoId) aSawB = true;
                    return bSawA && aSawB;
                }, 10000);
                Record("Proximity", ok ? "PASS" : "FAIL", $"BravoSawAlpha={bSawA}, AlphaSawBravo={aSawB}");
            }

            // 3 InstantMessage
            {
                string msg = "alpha-im-" + Guid.NewGuid();
                bool rx = false; UUID from = UUID.Zero;
                bravo.Client.Self.IM += (s, e) =>
                {
                    if (!rx && e.IM.Dialog == InstantMessageDialog.MessageFromAgent && e.IM.FromAgentID == alphaId && e.IM.Message == msg)
                    { rx = true; from = e.IM.FromAgentID; }
                };
                alpha.Client.Self.InstantMessage(bravoId, msg);
                bool ok = await WaitFor(() => rx, 5000);
                Record("InstantMessage", ok ? "PASS" : "FAIL", ok ? $"Received from {from}" : "Not received");
            }

            // 4 ObjectHandoff
            {
                string stage = "rezzed";
                bool pass = true;
                uint alphaLocal = 0; UUID primId = UUID.Zero; bool rezzed = false;
                alpha.Client.Objects.ObjectUpdate += (s, e) =>
                {
                    if (!rezzed && e.IsNew && e.Prim.OwnerID == alphaId && e.Prim.PrimData.PCode == PCode.Prim)
                    { rezzed = true; alphaLocal = e.Prim.LocalID; primId = e.Prim.ID; }
                };
                try
                {
                    Vector3 alphaPos = alpha.Client.Self.SimPosition;
                    var cd = new Primitive.ConstructionData { PCode = PCode.Prim, ProfileCurve = ProfileCurve.Square, PathCurve = PathCurve.Line, ProfileBegin = 0f, ProfileEnd = 1f, PathBegin = 0f, PathEnd = 1f, PathScaleX = 1f, PathScaleY = 1f, PathRevolutions = 1f, Material = Material.Wood };
                    alpha.Client.Objects.AddPrim(alpha.Client.Network.CurrentSim, cd, UUID.Zero, alphaPos + new Vector3(2, 0, 0), new Vector3(0.5f, 0.5f, 0.5f), Quaternion.Identity);
                }
                catch (Exception ex) { stage = "rez call"; pass = false; Console.WriteLine(ex); }
                if (pass) { if (!await WaitFor(() => rezzed, 10000)) { pass = false; stage = "rezzed"; } }

                bool visible = false, perms = false; uint bravoLocal = 0;
                bravo.Client.Objects.ObjectUpdate += (s, e) =>
                {
                    if (!visible && e.Prim != null && e.Prim.ID == primId) { visible = true; bravoLocal = e.Prim.LocalID; }
                };
                if (pass)
                {
                    try
                    {
                        alpha.Client.Objects.SetPermissions(alpha.Client.Network.CurrentSim, new List<uint> { alphaLocal }, PermissionWho.NextOwner, PermissionMask.Copy | PermissionMask.Modify | PermissionMask.Transfer, true);
                        alpha.Client.Objects.SetSaleInfo(alpha.Client.Network.CurrentSim, alphaLocal, SaleType.Copy, 0);
                        perms = true;
                    }
                    catch (Exception ex) { pass = false; stage = "perms set"; Console.WriteLine(ex); }
                    await Task.Delay(1500);
                    bool vis = await WaitFor(() =>
                    {
                        if (!visible)
                        {
                            try
                            {
                                dynamic sim = bravo.Client.Network.CurrentSim;
                                foreach (var val in sim.ObjectsPrims.Values)
                                    if (val.ID == primId) { visible = true; bravoLocal = (uint)val.LocalID; }
                            }
                            catch { }
                        }
                        return visible;
                    }, 10000);
                    if (!vis) { pass = false; stage = "visible to Bravo"; }
                }

                bool bought = false, received = false, purchaseStarted = false;
                bravo.Client.Inventory.Store.InventoryObjectAdded += (s, e) =>
                {
                    if (purchaseStarted && !received && e.Obj is InventoryItem) received = true;
                };
                if (pass)
                {
                    UUID folder = bravo.Client.Inventory.FindFolderForType(FolderType.Object);
                    if (folder == UUID.Zero) folder = bravo.Client.Inventory.Store.RootFolder.UUID;
                    try
                    {
                        bravo.Client.Objects.BuyObject(bravo.Client.Network.CurrentSim, bravoLocal, SaleType.Copy, 0, UUID.Zero, folder);
                        bought = true; purchaseStarted = true;
                    }
                    catch (Exception ex) { pass = false; stage = "bought"; Console.WriteLine(ex); }
                }
                if (pass && bought)
                {
                    bool got = await WaitFor(() => received, 10000);
                    if (!got) { pass = false; stage = "received"; }
                }

                string stages = $"rezzed={(rezzed ? "PASS" : "FAIL")}, perms set={(perms ? "PASS" : "FAIL")}, visible to Bravo={(visible ? "PASS" : "FAIL")}, bought={(bought ? "PASS" : "FAIL")}, received={(received ? "PASS" : "FAIL")}";
                string reason = pass ? stages : $"FAIL at stage '{stage}'; {stages}";
                Record("ObjectHandoff", pass ? "PASS" : "FAIL", reason);
            }

            // 5 GroupLifecycle
            {
                bool created = false; UUID groupId = UUID.Zero; string groupName = "MultiBot" + Guid.NewGuid().ToString("N").Substring(0, 8);
                alpha.Client.Groups.GroupCreatedReply += (s, e) =>
                {
                    if (!created) { groupId = e.GroupID; created = e.Success && e.GroupID != UUID.Zero; }
                };
                try { alpha.Client.Groups.RequestCreateGroup(new Group { Name = groupName, Charter = "test", ShowInList = true, AllowPublish = false, OpenEnrollment = false, MembershipFee = 0, MaturePublish = false }); }
                catch (Exception ex) { Console.WriteLine(ex); }
                bool cOk = await WaitFor(() => created, 10000);

                bool inviteSeen = false; UUID inviteTx = UUID.Zero;
                bravo.Client.Groups.GroupInvitation += (s, e) => { if (!inviteSeen) inviteSeen = true; };
                bravo.Client.Self.IM += (s, e) =>
                {
                    if (e.IM.Dialog == InstantMessageDialog.GroupInvitation)
                    { inviteSeen = true; if (inviteTx == UUID.Zero) inviteTx = e.IM.IMSessionID; }
                };
                bool invSent = false;
                if (cOk)
                {
                    try { alpha.Client.Groups.Invite(groupId, new List<UUID> { UUID.Zero }, bravoId); invSent = true; } catch (Exception ex) { Console.WriteLine(ex); }
                }
                bool invOk = cOk && await WaitFor(() => inviteSeen, 10000);
                if (invOk && inviteTx == UUID.Zero) await Task.Delay(1000);

                bool listed = false;
                bravo.Client.Groups.CurrentGroups += (s, e) => { if (e.Groups.ContainsKey(groupId)) listed = true; };
                bool joinReply = false;
                bravo.Client.Groups.GroupJoinedReply += (s, e) => { if (e.GroupID == groupId && e.Success) joinReply = true; };
                bool accepted = false;
                Console.WriteLine($"[GROUP] invite tx={inviteTx} seen={inviteSeen}");
                if (invOk)
                {
                    try { bravo.Client.Self.GroupInviteRespond(groupId, inviteTx, true); accepted = true; } catch (Exception ex) { Console.WriteLine(ex); }
                }
                bool joined = false;
                if (accepted)
                {
                    joined = await WaitFor(() => joinReply, 10000);
                    if (!joined)
                    {
                        joined = await WaitFor(() => listed, 5000);
                    }
                }

                bool chatA = false, chatB = false;
                alpha.Client.Self.GroupChatJoined += (s, e) => { if (e.Success) chatA = true; };
                bravo.Client.Self.GroupChatJoined += (s, e) => { if (e.Success) chatB = true; };
                if (joined)
                {
                    try { alpha.Client.Self.RequestJoinGroupChat(groupId); } catch (Exception ex) { Console.WriteLine(ex); }
                    try { bravo.Client.Self.RequestJoinGroupChat(groupId); } catch (Exception ex) { Console.WriteLine(ex); }
                }
                bool chatOkA = false, chatOkB = false;
                if (joined)
                {
                    chatOkA = await WaitFor(() => chatA, 8000);
                    chatOkB = await WaitFor(() => chatB, 8000);
                }
                await Task.Delay(1500);

                string gmsg = "group-msg-" + Guid.NewGuid();
                bool gReceived = false;
                bravo.Client.Self.IM += (s, e) => { if (!gReceived && e.IM.Dialog == InstantMessageDialog.SessionSend && e.IM.Message == gmsg) gReceived = true; };
                bool sent = false;
                if (chatOkA && chatOkB)
                {
                    try { alpha.Client.Self.InstantMessageGroup(groupId, gmsg); sent = true; } catch (Exception ex) { Console.WriteLine(ex); }
                }
                bool msgOk = sent && await WaitFor(() => gReceived, 8000);

                bool overall = cOk && invOk && joined && chatOkA && chatOkB && msgOk;
                string stages = $"created={(cOk ? "PASS" : "FAIL")}, inviteSeen={(invOk ? "PASS" : "FAIL")}, joined={(joined ? "PASS" : "FAIL")}, chatA={(chatOkA ? "PASS" : "FAIL")}, chatB={(chatOkB ? "PASS" : "FAIL")}, msg={(msgOk ? "PASS" : "FAIL")}";
                string firstFail = !cOk ? "created" : !invOk ? "invite seen" : !joined ? "joined" : !chatOkA ? "chat joined A" : !chatOkB ? "chat joined B" : !msgOk ? "message received" : null;
                string reason = overall ? stages : $"FAIL at stage '{firstFail}'; {stages}";
                Record("GroupLifecycle", overall ? "PASS" : "FAIL", reason);
            }

            // 6 HumanCoexistence
            {
                var avatars = alpha.Client.Network.CurrentSim.ObjectsAvatars.Values.ToList();
                Console.WriteLine($"Alpha sees {avatars.Count} avatars:");
                foreach (var a in avatars) Console.WriteLine($"  {a.Name} ({a.ID})");
                int humans = avatars.Count(a => !a.Name.Contains("Bot "));
                Console.WriteLine($"Human viewers present: {humans}");
                bool conn = alpha.Client.Network.Connected && bravo.Client.Network.Connected;
                Record("HumanCoexistence", conn ? "PASS" : "FAIL", $"avatars={avatars.Count}, humans={humans}, AlphaConnected={alpha.Client.Network.Connected}, BravoConnected={bravo.Client.Network.Connected}");
            }
        }
        catch (Exception ex) { Console.WriteLine($"Unexpected error: {ex}"); }
        finally
        {
            try { alpha?.Client?.Network?.Logout(); } catch { }
            try { bravo?.Client?.Network?.Logout(); } catch { }
        }

        Console.WriteLine();
        Console.WriteLine("SUMMARY");
        Console.WriteLine($"{"Test",-22} {"Result",-6} Reason");
        foreach (var r in Outcomes) Console.WriteLine($"{r.Name,-22} {r.Status,-6} {r.Reason}");
        return Outcomes.All(r => r.Status == "PASS") ? 0 : 1;
    }
}
