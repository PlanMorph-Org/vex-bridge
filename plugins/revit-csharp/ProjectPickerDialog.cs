using System;
using System.Windows.Forms;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Minimal modal dialog asking the user which architur project to push to
/// and which branch. Replaces the original env-var (VEX_BRIDGE_PROJECT_ID)
/// which made marketplace submission impossible. We deliberately avoid
/// WPF/XAML so the plugin stays in a single-binary .NET Framework 4.8 build.
/// </summary>
internal sealed class ProjectPickerDialog : Form
{
    private readonly TextBox _projectBox;
    private readonly TextBox _branchBox;
    private readonly Button _ok;
    private readonly Button _cancel;

    public string ProjectId => _projectBox.Text.Trim();
    public string Branch => string.IsNullOrWhiteSpace(_branchBox.Text) ? "main" : _branchBox.Text.Trim();

    public ProjectPickerDialog(string? defaultProjectId = null)
    {
        Text = "Push to architur";
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = false;
        MaximizeBox = false;
        Width = 460;
        Height = 220;

        var lblP = new Label { Left = 16, Top = 16, Width = 120, Text = "Project ID" };
        _projectBox = new TextBox { Left = 140, Top = 14, Width = 280, Text = defaultProjectId ?? "" };

        var lblB = new Label { Left = 16, Top = 56, Width = 120, Text = "Branch" };
        _branchBox = new TextBox { Left = 140, Top = 54, Width = 280, Text = "main" };

        var hint = new Label
        {
            Left = 16, Top = 90, Width = 408, Height = 40,
            ForeColor = System.Drawing.SystemColors.GrayText,
            Text = "Find the project ID on studio.planmorph.software under Project ▸ Settings. " +
                   "Leave branch as 'main' for normal saves.",
        };

        _ok = new Button { Left = 240, Top = 140, Width = 80, Text = "Push", DialogResult = DialogResult.OK };
        _cancel = new Button { Left = 340, Top = 140, Width = 80, Text = "Cancel", DialogResult = DialogResult.Cancel };
        AcceptButton = _ok;
        CancelButton = _cancel;

        Controls.AddRange(new Control[] { lblP, _projectBox, lblB, _branchBox, hint, _ok, _cancel });
    }
}
