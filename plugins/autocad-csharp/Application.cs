// VexBridgeAutoCAD: registers itself as an AutoCAD extension and exposes
// two commands on the command line:
//   VEXPUSH  — push the active drawing to architur
//   VEXPAIR  — pair this device with an architur account
//
// Auto-loaded by AutoCAD via the .bundle / PackageContents.xml mechanism
// in %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\.
using System;
using System.Reflection;
using Autodesk.AutoCAD.ApplicationServices;
using Autodesk.AutoCAD.Runtime;
// Disambiguate against Autodesk.AutoCAD.Runtime types.
using AcadApp = Autodesk.AutoCAD.ApplicationServices.Application;

[assembly: ExtensionApplication(typeof(Architur.VexBridgeAutoCAD.VexBridgeApplication))]
[assembly: CommandClass(typeof(Architur.VexBridgeAutoCAD.Commands))]

namespace Architur.VexBridgeAutoCAD;

/// <summary>
/// Loaded by AutoCAD on startup. Opportunistically launches the bundled
/// vex-bridge daemon so a freshly-installed user never has to log out
/// and back in for the per-user Scheduled Task to fire — the daemon
/// comes up the first time they open AutoCAD after installing.
/// All work is fire-and-forget; we never block AutoCAD boot.
/// </summary>
public sealed class VexBridgeApplication : IExtensionApplication
{
    public void Initialize()
    {
        try
        {
            System.Threading.Tasks.Task.Run(() => BundledBin.EnsureDaemonRunning());
            var doc = AcadApp.DocumentManager.MdiActiveDocument;
            doc?.Editor.WriteMessage(
                "\nvex-bridge loaded. Type VEXPUSH to push, VEXPAIR to pair this device.\n");
        }
        catch
        {
            // Never crash AutoCAD because the bridge couldn't start.
        }
    }

    public void Terminate()
    {
        // Daemon is a separate process owned by Task Scheduler; nothing
        // for the plugin to clean up at AutoCAD exit.
    }
}
