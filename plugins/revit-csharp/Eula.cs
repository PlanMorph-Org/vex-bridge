using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace Architur.VexBridgeRevit;

/// <summary>
/// First-run EULA gate. The Autodesk App Store explicitly requires apps
/// to display a EULA the user must accept before doing any work, and to
/// allow them to re-display it later (PairCommand exposes that path
/// indirectly; users can also delete %APPDATA%\vex-bridge\eula-accepted
/// to force the dialog again). Acceptance is recorded per-user in a
/// marker file under %APPDATA%\vex-bridge\.
/// </summary>
internal static class Eula
{
    public static bool EnsureAccepted()
    {
        if (HasAccepted()) return true;
        return ShowDialogInternal();
    }

    public static void ShowDialogAlways() => ShowDialogInternal();

    private static bool HasAccepted() => File.Exists(MarkerFile());

    private static void RecordAcceptance()
    {
        var path = MarkerFile();
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, $"accepted={DateTime.UtcNow:O}\nversion={EulaVersion()}\n");
    }

    private static string MarkerFile() =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "vex-bridge",
            "eula-accepted");

    private static string EulaVersion() =>
        typeof(Eula).Assembly.GetName().Version?.ToString() ?? "unknown";

    private static bool ShowDialogInternal()
    {
        using var f = new Form
        {
            Text = "vex-bridge — End User License Agreement",
            Width = 720,
            Height = 540,
            StartPosition = FormStartPosition.CenterParent,
            FormBorderStyle = FormBorderStyle.FixedDialog,
            MinimizeBox = false,
            MaximizeBox = false,
        };
        var box = new TextBox
        {
            Multiline = true,
            ReadOnly = true,
            ScrollBars = ScrollBars.Vertical,
            Dock = DockStyle.Top,
            Height = 420,
            Font = new Font(FontFamily.GenericSansSerif, 9f),
            Text = DefaultEulaText,
        };
        var accept  = new Button { Text = "Accept",  DialogResult = DialogResult.OK,     Width = 100, Top = 440, Left = 480 };
        var decline = new Button { Text = "Decline", DialogResult = DialogResult.Cancel, Width = 100, Top = 440, Left = 590 };
        f.Controls.AddRange(new Control[] { box, accept, decline });
        f.AcceptButton = accept;
        f.CancelButton = decline;

        var result = f.ShowDialog();
        if (result == DialogResult.OK) { RecordAcceptance(); return true; }
        return false;
    }

    private const string DefaultEulaText =
@"vex-bridge — End User License Agreement (Apache License 2.0)

Copyright Architur. Licensed under the Apache License, Version 2.0
(the ""License""); you may not use this software except in compliance
with the License. You may obtain a copy at:

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an ""AS IS"" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
implied. See the License for the specific language governing
permissions and limitations under the License.

Privacy: vex-bridge runs entirely on your machine. It transmits only
the files you explicitly push, and only to the architur server you
have paired with. The full source is published at:

    https://github.com/PlanMorph-Org/vex-bridge

Click Accept to continue, or Decline to cancel. To re-display this
dialog later, delete %APPDATA%\vex-bridge\eula-accepted.";
}
