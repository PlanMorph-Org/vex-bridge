using System;
using System.Diagnostics;
using System.Threading;
using Autodesk.Revit.Attributes;
using Autodesk.Revit.UI;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Drives the full pairing flow without ever opening a console window:
///   1. Make sure the bundled daemon is running (start it hidden if not).
///   2. POST /v1/pair/start to get a code + verification URL.
///   3. Open the user's default browser at the verification URL.
///   4. Show a TaskDialog with the code (so they can confirm it matches
///      what they see in the browser) and a Cancel button.
///   5. Poll /v1/pair/status on a background thread until paired (or
///      until the user cancels / the request expires).
///
/// This is the entry point that makes the App-Store-installed plug-in
/// usable for an architect who has never opened a terminal in their life.
/// </summary>
[Transaction(TransactionMode.ReadOnly)]
[Regeneration(RegenerationOption.Manual)]
public sealed class PairCommand : IExternalCommand
{
    public Result Execute(ExternalCommandData data, ref string message, Autodesk.Revit.DB.ElementSet elements)
    {
        try
        {
            BundledBin.EnsureDaemonRunning();

            using var bridge = BridgeClient.TryCreate();
            if (bridge is null)
            {
                TaskDialog.Show(
                    "vex-bridge",
                    "Could not contact the local vex-bridge service.\n\n" +
                    "Try restarting Revit. If the problem persists, reinstall the " +
                    "plug-in from the Autodesk App Store.");
                return Result.Failed;
            }

            if (bridge.IsPaired())
            {
                TaskDialog.Show("vex-bridge",
                    "This machine is already paired with your architur account.\n\n" +
                    "Use the Push button to send the active model.");
                return Result.Succeeded;
            }

            var deviceLabel = $"Revit on {Environment.MachineName}";
            var start       = bridge.StartPair(deviceLabel);

            // Open the verification URL in the default browser. UseShellExecute=true
            // hands off to the OS — no terminal window appears.
            try
            {
                var psi = new ProcessStartInfo(start.PairUrl) { UseShellExecute = true };
                Process.Start(psi);
            }
            catch { /* fall back to the URL in the dialog */ }

            using var cts = new CancellationTokenSource();
            var pairedFlag = false;

            var worker = new System.Threading.Thread(() =>
            {
                pairedFlag = bridge.WaitForPaired(cts.Token, TimeSpan.FromMinutes(10));
            }) { IsBackground = true };
            worker.Start();

            // Modal "waiting" dialog. Cancel signals the worker.
            var td = new TaskDialog("vex-bridge — pairing")
            {
                MainInstruction      = $"Confirm code in your browser: {start.Code}",
                MainContent          =
                    "A browser tab has opened on architur.com. Approve this device " +
                    "to finish pairing.\n\n" +
                    "If the tab did not open, copy this URL into your browser:\n" +
                    start.PairUrl,
                CommonButtons        = TaskDialogCommonButtons.Cancel,
                AllowCancellation    = true,
                DefaultButton        = TaskDialogResult.Cancel,
            };
            var result = td.Show();
            if (result == TaskDialogResult.Cancel)
            {
                cts.Cancel();
                worker.Join(TimeSpan.FromSeconds(3));
                return Result.Cancelled;
            }

            // User dismissed the dialog without cancelling — wait briefly
            // for the worker to flip the flag.
            worker.Join(TimeSpan.FromSeconds(2));
            if (pairedFlag)
            {
                TaskDialog.Show("vex-bridge", "Paired. You can now use the Push button.");
                return Result.Succeeded;
            }

            TaskDialog.Show("vex-bridge",
                "Pairing didn't complete. Click Pair again to retry.");
            return Result.Cancelled;
        }
        catch (BridgeException bex)
        {
            TaskDialog.Show("vex-bridge", $"Pairing failed: HTTP {bex.StatusCode}\n\n{bex.Message}");
            return Result.Failed;
        }
        catch (Exception ex)
        {
            TaskDialog.Show("vex-bridge", $"Pairing failed.\n\n{ex.Message}");
            return Result.Failed;
        }
    }
}
