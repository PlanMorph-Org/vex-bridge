using System;
using System.Diagnostics;
using System.Threading;
using Autodesk.AutoCAD.ApplicationServices;
using Autodesk.AutoCAD.Runtime;

namespace Architur.VexBridgeAutoCAD;

/// <summary>
/// AutoCAD command handlers. AutoCAD invokes any static or instance
/// method tagged with [CommandMethod] when the user types the matching
/// command name (or runs it from a toolbar/menu).
///
/// VEXPUSH  — prompts for project ID + branch, pushes the active drawing.
/// VEXPAIR  — runs the full first-time pairing flow in-process.
/// VEXEULA  — re-shows the EULA acceptance dialog (App Store requirement).
/// </summary>
public sealed class Commands
{
    private const string LastProjectKey = "VEX_BRIDGE_LAST_PROJECT_ID";

    [CommandMethod("VEXPUSH", CommandFlags.Modal)]
    public void Push()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var ed  = doc?.Editor;
        if (ed is null) return;

        try
        {
            if (!Eula.EnsureAccepted()) { ed.WriteMessage("\nVEXPUSH cancelled — EULA not accepted.\n"); return; }
            BundledBin.EnsureDaemonRunning();

            using var bridge = BridgeClient.TryCreate();
            if (bridge is null)
            {
                ed.WriteMessage("\nCould not contact the local vex-bridge service. " +
                                "Restart AutoCAD; reinstall the plug-in if it persists.\n");
                return;
            }
            if (!bridge.IsPaired())
            {
                ed.WriteMessage("\nThis machine is not paired with an architur account. " +
                                "Run VEXPAIR to pair it — no terminal needed.\n");
                return;
            }

            var last = Environment.GetEnvironmentVariable(LastProjectKey);
            using var dlg = new ProjectPickerDialog(last);
            if (Application.ShowModalDialog(dlg) != System.Windows.Forms.DialogResult.OK)
            {
                ed.WriteMessage("\nVEXPUSH cancelled.\n");
                return;
            }

            // Cache the last-used project id for next time. Per-user env var,
            // not machine-wide.
            Environment.SetEnvironmentVariable(LastProjectKey, dlg.ProjectId,
                                               EnvironmentVariableTarget.User);

            // Idempotent register before push, so a brand-new project works
            // on the very first try without manual config edits.
            bridge.Register(dlg.ProjectId);
            var text = bridge.Push(dlg.ProjectId, dlg.Branch);
            ed.WriteMessage($"\nPushed to architur.\n{text}\n");
        }
        catch (BridgeException bex)
        {
            ed.WriteMessage($"\nPush failed: HTTP {bex.StatusCode}\n{bex.Message}\n");
        }
        catch (Exception ex)
        {
            ed.WriteMessage($"\nPush failed: {ex.Message}\n");
        }
    }

    [CommandMethod("VEXPAIR", CommandFlags.Modal)]
    public void Pair()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var ed  = doc?.Editor;
        if (ed is null) return;

        try
        {
            if (!Eula.EnsureAccepted()) { ed.WriteMessage("\nVEXPAIR cancelled — EULA not accepted.\n"); return; }
            BundledBin.EnsureDaemonRunning();

            using var bridge = BridgeClient.TryCreate();
            if (bridge is null)
            {
                ed.WriteMessage("\nCould not contact the local vex-bridge service.\n");
                return;
            }
            if (bridge.IsPaired())
            {
                ed.WriteMessage("\nThis machine is already paired. Use VEXPUSH to send the drawing.\n");
                return;
            }

            var deviceLabel = $"AutoCAD on {Environment.MachineName}";
            var start       = bridge.StartPair(deviceLabel);

            // ShellExecute hands off to the OS — no console window appears.
            try { Process.Start(new ProcessStartInfo(start.PairUrl) { UseShellExecute = true }); }
            catch { /* fall back to printing the URL below */ }

            ed.WriteMessage($"\nPairing code: {start.Code}\n" +
                            $"Approve in your browser: {start.PairUrl}\n" +
                            $"(opening browser automatically; type CANCEL to abort)\n");

            using var cts = new CancellationTokenSource();
            var paired = bridge.WaitForPaired(cts.Token, TimeSpan.FromMinutes(10));
            ed.WriteMessage(paired
                ? "\nPaired. Use VEXPUSH to send the active drawing.\n"
                : "\nPairing didn't complete. Run VEXPAIR again to retry.\n");
        }
        catch (BridgeException bex)
        {
            ed.WriteMessage($"\nPairing failed: HTTP {bex.StatusCode}\n{bex.Message}\n");
        }
        catch (Exception ex)
        {
            ed.WriteMessage($"\nPairing failed: {ex.Message}\n");
        }
    }

    [CommandMethod("VEXEULA", CommandFlags.Modal)]
    public void ShowEula()
    {
        Eula.ShowDialogAlways();
    }
}
