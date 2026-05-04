using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Locates the binaries we ship beside the Revit add-in. Supported layouts:
///   C:\Program Files\Autodesk\Revit {year}\AddIns\VexBridge\VexBridgeRevit.dll
///   C:\Program Files\Autodesk\Revit {year}\AddIns\VexBridge\bin\
/// and the legacy Autodesk bundle layout:
///   %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\Revit\{year}\VexBridgeRevit.dll
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
        // 1. Sibling bin\ inside the Revit AddIns payload.
        //    DLL path: …\AddIns\VexBridge\VexBridgeRevit.dll
        //    Bin path: …\AddIns\VexBridge\bin\{fileName}
        var asm = Assembly.GetExecutingAssembly().Location;
        if (!string.IsNullOrEmpty(asm))
        {
            var addInDir = Path.GetDirectoryName(asm);
            if (!string.IsNullOrEmpty(addInDir))
            {
                var localBin = Path.Combine(addInDir, "bin", fileName);
                if (File.Exists(localBin)) return localBin;
            }
        }

        // 2. Sibling Contents\bin\ inside the Autodesk bundle.
        //    DLL path: …\Contents\Revit\{year}\VexBridgeRevit.dll
        //    Bin path: …\Contents\bin\{fileName}
        // Walk up looking for a directory named "Contents" so the lookup
        // keeps working if the bundle layout changes (e.g. legacy
        // Contents\{year}\ from <0.2.8 still works for in-place upgrades).
        if (!string.IsNullOrEmpty(asm))
        {
            var dir = Path.GetDirectoryName(asm);
            for (var i = 0; i < 6 && !string.IsNullOrEmpty(dir); i++)
            {
                if (string.Equals(Path.GetFileName(dir), "Contents", StringComparison.OrdinalIgnoreCase))
                {
                    var bundled = Path.Combine(dir, "bin", fileName);
                    if (File.Exists(bundled)) return bundled;
                    break;
                }
                dir = Path.GetDirectoryName(dir);
            }
        }

        // 3. Fall back to whatever's on PATH.
        return fileName;
    }
}
