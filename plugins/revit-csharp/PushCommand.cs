using System;
using System.IO;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Autodesk.Revit.Attributes;
using Autodesk.Revit.UI;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Reads the access token from disk, POSTs to the local vex-bridge daemon,
/// and shows the result in a TaskDialog. Intentionally tiny — all real work
/// (auth, vex CLI, SSH, network) is in the daemon.
/// </summary>
[Transaction(TransactionMode.ReadOnly)]
[Regeneration(RegenerationOption.Manual)]
public sealed class PushCommand : IExternalCommand
{
    private const string BridgeUrl = "http://127.0.0.1:7878/v1/repo/push";

    public Result Execute(ExternalCommandData data, ref string message, Autodesk.Revit.DB.ElementSet elements)
    {
        try
        {
            var token = ReadToken();
            if (string.IsNullOrEmpty(token))
            {
                TaskDialog.Show(
                    "vex-bridge",
                    "vex-bridge is not running, or this machine is not paired.\n\n" +
                    "Open a terminal and run:  vex-bridge pair");
                return Result.Cancelled;
            }

            // TODO: prompt the user (or read from a project parameter) for project_id.
            // For now we accept it via an env var so the plumbing is testable.
            var projectId = Environment.GetEnvironmentVariable("VEX_BRIDGE_PROJECT_ID");
            if (string.IsNullOrEmpty(projectId))
            {
                TaskDialog.Show("vex-bridge", "Set the VEX_BRIDGE_PROJECT_ID env var first.");
                return Result.Cancelled;
            }

            var body = JsonSerializer.Serialize(new { project_id = projectId, branch = "main" });
            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(2) };
            http.DefaultRequestHeaders.Add("X-Vex-Bridge-Token", token);
            using var content = new StringContent(body, Encoding.UTF8, "application/json");

            // Block intentionally — Revit commands are synchronous.
            var resp = http.PostAsync(BridgeUrl, content).GetAwaiter().GetResult();
            var text = resp.Content.ReadAsStringAsync().GetAwaiter().GetResult();
            TaskDialog.Show("vex-bridge",
                resp.IsSuccessStatusCode ? $"Pushed.\n\n{text}" : $"Push failed: {(int)resp.StatusCode}\n{text}");
            return resp.IsSuccessStatusCode ? Result.Succeeded : Result.Failed;
        }
        catch (Exception ex)
        {
            message = ex.Message;
            return Result.Failed;
        }
    }

    private static string? ReadToken()
    {
        var path = Environment.OSVersion.Platform == PlatformID.Win32NT
            ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                           "vex-bridge", "access-token")
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                           ".config", "vex-bridge", "access-token");
        return File.Exists(path) ? File.ReadAllText(path).Trim() : null;
    }
}
