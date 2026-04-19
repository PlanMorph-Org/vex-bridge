using System;
using Autodesk.Revit.UI;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Adds a single "Push to architur" button to a "vex-bridge" ribbon panel.
/// Everything else is in <see cref="PushCommand"/>.
/// </summary>
public sealed class VexBridgeApplication : IExternalApplication
{
    public Result OnStartup(UIControlledApplication app)
    {
        var panel = app.CreateRibbonPanel("vex-bridge");
        var asm = typeof(VexBridgeApplication).Assembly.Location;
        var btnData = new PushButtonData(
            name:        "VexBridgePush",
            text:        "Push to\narchitur",
            assemblyName: asm,
            className:   typeof(PushCommand).FullName);
        btnData.ToolTip = "Commit + push the current model to architur.";
        panel.AddItem(btnData);
        return Result.Succeeded;
    }

    public Result OnShutdown(UIControlledApplication _) => Result.Succeeded;
}
