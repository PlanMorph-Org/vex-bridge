using System;
using Autodesk.Revit.UI;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Adds two ribbon buttons to a "vex-bridge" panel:
///   * Push to architur — commit + push the current model
///   * Pair this device — kicks off the daemon's pairing flow (in-process)
///
/// On Revit startup we also opportunistically start the bundled
/// vex-bridge daemon if it isn't already running. This means a freshly
/// installed user does not have to log out / log back in for the per-user
/// Scheduled Task to fire — the daemon comes up the first time they
/// launch Revit after installing the MSI. The call is non-blocking and
/// swallows all exceptions.
/// </summary>
public sealed class VexBridgeApplication : IExternalApplication
{
    public Result OnStartup(UIControlledApplication app)
    {
        // Fire-and-forget; never block Revit boot on this.
        try { System.Threading.Tasks.Task.Run(() => BundledBin.EnsureDaemonRunning()); }
        catch { /* never crash Revit because the bridge couldn't start */ }

        var panel = app.CreateRibbonPanel("vex-bridge");
        var asm = typeof(VexBridgeApplication).Assembly.Location;

        var pushBtn = new PushButtonData(
            name:        "VexBridgePush",
            text:        "Push to\narchitur",
            assemblyName: asm,
            className:   typeof(PushCommand).FullName)
        {
            ToolTip = "Commit + push the current model to architur.",
            LongDescription = "Sends the active document to architur via the local " +
                              "vex-bridge daemon. Bundled with the add-in — no separate install.",
        };

        var pairBtn = new PushButtonData(
            name:        "VexBridgePair",
            text:        "Pair this\ndevice",
            assemblyName: asm,
            className:   typeof(PairCommand).FullName)
        {
            ToolTip = "Pair this machine with your architur account.",
        };

        panel.AddItem(pushBtn);
        panel.AddItem(pairBtn);
        return Result.Succeeded;
    }

    public Result OnShutdown(UIControlledApplication _) => Result.Succeeded;
}
