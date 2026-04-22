using System;
using Autodesk.Revit.Attributes;
using Autodesk.Revit.UI;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Reads the local access token, prompts the user for the architur project
/// id + branch, and POSTs to the local vex-bridge daemon. All real work
/// (auth, vex CLI, SSH, network) is in the daemon.
///
/// Auto-starts the bundled daemon if it isn't running yet, so a fresh
/// install never shows a "service not running" dialog.
/// </summary>
[Transaction(TransactionMode.ReadOnly)]
[Regeneration(RegenerationOption.Manual)]
public sealed class PushCommand : IExternalCommand
{
    private const string LastProjectKey = "VEX_BRIDGE_LAST_PROJECT_ID";

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
                    "Try restarting Revit. If this keeps happening, reinstall " +
                    "the plug-in from the Autodesk App Store.");
                return Result.Cancelled;
            }
            if (!bridge.IsPaired())
            {
                TaskDialog.Show("vex-bridge",
                    "This machine is not paired with an architur account.\n\n" +
                    "Click the Pair button on the ribbon to pair it — no terminal needed.");
                return Result.Cancelled;
            }

            var last = Environment.GetEnvironmentVariable(LastProjectKey);
            using var dlg = new ProjectPickerDialog(last);
            if (dlg.ShowDialog() != System.Windows.Forms.DialogResult.OK)
                return Result.Cancelled;

            // Cache the last-used project id for next time. Per-user env var
            // — written to the user's environment block, not the machine.
            Environment.SetEnvironmentVariable(LastProjectKey, dlg.ProjectId, EnvironmentVariableTarget.User);

            // Idempotently register the project before pushing. This is what
            // makes the Push button work on the *first* time the user opens
            // a brand-new project — without this, the daemon would 404
            // because no [[watch]] entry exists in config.toml yet. Safe to
            // call every time: the daemon replaces the existing entry.
            bridge.Register(dlg.ProjectId);

            var text = bridge.Push(dlg.ProjectId, dlg.Branch);
            TaskDialog.Show("vex-bridge", $"Pushed.\n\n{text}");
            return Result.Succeeded;
        }
        catch (BridgeException bex)
        {
            TaskDialog.Show("vex-bridge", $"Push failed: HTTP {bex.StatusCode}\n\n{bex.Message}");
            return Result.Failed;
        }
        catch (Exception ex)
        {
            message = ex.Message;
            return Result.Failed;
        }
    }
}
