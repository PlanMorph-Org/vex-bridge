using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;

namespace Architur.VexBridgeAutoCAD;

/// <summary>
/// Locates the binaries shipped inside the Autodesk bundle. The AutoCAD
/// add-in is loaded from:
///   %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\AutoCAD\{year}\VexBridgeAutoCAD.dll
/// so the bundled CLI + daemon live two directories up at:
///   %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\bin\
/// (Same bin/ folder that the Revit plug-in uses — there's only one daemon.)
/// </summary>
internal static class BundledBin
{
    public static string VexBridgeExe => Resolve("vex-bridge.exe");
    public static string VexExe       => Resolve("vex.exe");

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
        if (string.IsNullOrEmpty(exe)) return;

        var psi = new ProcessStartInfo(exe, "start")
        {
            UseShellExecute        = false,
            CreateNoWindow         = true,
            WindowStyle            = ProcessWindowStyle.Hidden,
            RedirectStandardOutput = false,
            RedirectStandardError  = false,
        };
        try { Process.Start(psi); } catch { /* best-effort */ }

        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (DateTime.UtcNow < deadline)
        {
            if (IsDaemonRunning()) return;
            System.Threading.Thread.Sleep(150);
        }
    }

    private static string Resolve(string fileName)
    {
        var asm = Assembly.GetExecutingAssembly().Location;
        if (!string.IsNullOrEmpty(asm))
        {
            // …\Contents\AutoCAD\{year}\VexBridgeAutoCAD.dll → …\Contents\bin\
            var yearDir    = Path.GetDirectoryName(asm);
            var productDir = Path.GetDirectoryName(yearDir);
            var contentsDir = Path.GetDirectoryName(productDir);
            if (!string.IsNullOrEmpty(contentsDir))
            {
                var bundled = Path.Combine(contentsDir, "bin", fileName);
                if (File.Exists(bundled)) return bundled;
            }
        }
        return fileName;
    }
}
