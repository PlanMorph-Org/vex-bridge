using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Locates the binaries we ship inside the Autodesk bundle. The Revit
/// add-in is loaded from:
///   %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\{year}\VexBridgeRevit.dll
/// so the bundled CLI + daemon live one directory up at:
///   %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\bin\
///
/// Falls back to whatever's on PATH (named `vex.exe` / `vex-bridge.exe`)
/// for developer machines that ran build-bundle.ps1 on the file system
/// directly without going through the MSI.
/// </summary>
internal static class BundledBin
{
    public static string VexBridgeExe => Resolve("vex-bridge.exe");
    public static string VexExe       => Resolve("vex.exe");

    /// <summary>
    /// Best-effort: returns true if vex-bridge appears to be running by
    /// pinging the loopback /v1/health endpoint with a short timeout.
    /// </summary>
    public static bool IsDaemonRunning()
    {
        try
        {
            using var http = new System.Net.Http.HttpClient
            {
                Timeout = TimeSpan.FromMilliseconds(800),
            };
            var r = http.GetAsync("http://127.0.0.1:7878/v1/health").GetAwaiter().GetResult();
            return r.IsSuccessStatusCode;
        }
        catch { return false; }
    }

    /// <summary>
    /// Starts the bundled vex-bridge daemon detached, with NO console
    /// window. Idempotent: returns immediately if a daemon is already
    /// answering on 127.0.0.1:7878.
    /// </summary>
    public static void EnsureDaemonRunning()
    {
        if (IsDaemonRunning()) return;

        var exe = VexBridgeExe;
        if (string.IsNullOrEmpty(exe)) return; // best-effort

        var psi = new ProcessStartInfo(exe, "start")
        {
            UseShellExecute        = false,
            CreateNoWindow         = true,
            WindowStyle            = ProcessWindowStyle.Hidden,
            RedirectStandardOutput = false,
            RedirectStandardError  = false,
        };
        try { Process.Start(psi); } catch { /* best-effort */ }

        // Give it a moment to bind 127.0.0.1:7878. We poll instead of
        // sleeping a fixed time so a fast daemon doesn't waste a second.
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (DateTime.UtcNow < deadline)
        {
            if (IsDaemonRunning()) return;
            System.Threading.Thread.Sleep(150);
        }
    }

    private static string Resolve(string fileName)
    {
        // 1. Sibling Contents\bin\ inside the Autodesk bundle.
        var asm = Assembly.GetExecutingAssembly().Location;
        if (!string.IsNullOrEmpty(asm))
        {
            // …\Contents\{year}\VexBridgeRevit.dll  →  …\Contents\bin\
            var contentsDir = Path.GetDirectoryName(Path.GetDirectoryName(asm));
            if (!string.IsNullOrEmpty(contentsDir))
            {
                var bundled = Path.Combine(contentsDir, "bin", fileName);
                if (File.Exists(bundled)) return bundled;
            }
        }

        // 2. Fall back to whatever's on PATH.
        return fileName;
    }
}
