# ─────────────────────────────────────────────────────────────────────────
# Vex Atlas — Inno Setup wizard images
#
# Generates two BMPs that Inno Setup paints into the wizard chrome:
#
#   wizard.bmp        164×314 — left-edge panel on Welcome / Finish pages.
#                               Dark canvas + the accent wordmark, so the
#                               first impression of the installer matches
#                               studio.planmorph.software.
#   wizard-small.bmp   55× 58 — the little badge in the top-right of every
#                               other page. Solid accent square with a
#                               minimal mark.
#
# We render at build time (rather than committing BMPs) because:
#   • BMPs are awkward to keep in git diffs;
#   • we want the brand colours to live in *one* place — this script.
#
# Run from the same directory the .iss lives in:
#   pwsh -File gen-images.ps1
# ─────────────────────────────────────────────────────────────────────────

[CmdletBinding()]
param(
  [string]$OutDir = $PSScriptRoot
)

Add-Type -AssemblyName System.Drawing

# Brand palette — keep in sync with web/tailwind.config.ts
$Canvas       = [System.Drawing.Color]::FromArgb(0x0b, 0x0c, 0x0f)
$CanvasDeep   = [System.Drawing.Color]::FromArgb(0x06, 0x07, 0x09)
$Surface      = [System.Drawing.Color]::FromArgb(0x14, 0x16, 0x1b)
$Accent       = [System.Drawing.Color]::FromArgb(0xd4, 0xa5, 0x74)
$AccentStrong = [System.Drawing.Color]::FromArgb(0xe8, 0xb8, 0x85)
$FgMuted      = [System.Drawing.Color]::FromArgb(0x9a, 0xa0, 0xa6)
$Border       = [System.Drawing.Color]::FromArgb(0x2a, 0x2d, 0x35)

# ── Large panel ────────────────────────────────────────────────────────
# Inno Setup expects 164×314 (96 DPI) for WizardImageFile.
$bmp = New-Object System.Drawing.Bitmap 164, 314
$g   = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

# Vertical gradient: deep canvas top → canvas bottom. Subtle but kills the
# flat-rectangle look the default installer ships with.
$gradRect = New-Object System.Drawing.Rectangle 0, 0, 164, 314
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
  $gradRect, $CanvasDeep, $Canvas,
  [System.Drawing.Drawing2D.LinearGradientMode]::Vertical
)
$g.FillRectangle($grad, $gradRect)
$grad.Dispose()

# Faint blueprint grid — 16 px squares. Reads as "engineering tool"
# without competing with the wordmark.
$gridPen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(18, 255, 255, 255)), 1
for ($x = 0; $x -le 164; $x += 16) { $g.DrawLine($gridPen, $x, 0, $x, 314) }
for ($y = 0; $y -le 314; $y += 16) { $g.DrawLine($gridPen, 0, $y, 164, $y) }
$gridPen.Dispose()

# Accent rule down the left edge — picks up the brand colour the moment
# the wizard opens.
$accentBrush = New-Object System.Drawing.SolidBrush $Accent
$g.FillRectangle($accentBrush, 0, 0, 4, 314)
$accentBrush.Dispose()

# Wordmark — display serif vibe with system fallback. Inno's wizard panel
# is portrait, so we stack "VEX" / "ATLAS" tightly.
$titleFont = New-Object System.Drawing.Font 'Segoe UI Semibold', 22, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$subFont   = New-Object System.Drawing.Font 'Segoe UI', 9, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
$tinyFont  = New-Object System.Drawing.Font 'Consolas', 8, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)

$accentText = New-Object System.Drawing.SolidBrush $AccentStrong
$fgText     = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(0xee, 0xee, 0xf0))
$mutedText  = New-Object System.Drawing.SolidBrush $FgMuted

$g.DrawString('VEX',   $titleFont, $accentText, 16, 36)
$g.DrawString('ATLAS', $titleFont, $fgText,     16, 64)

# Hairline separator under the wordmark.
$rulePen = New-Object System.Drawing.Pen $Border, 1
$g.DrawLine($rulePen, 16, 100, 132, 100)
$rulePen.Dispose()

$g.DrawString("Version control",   $subFont, $mutedText, 16, 110)
$g.DrawString("for IFC and Revit", $subFont, $mutedText, 16, 124)

# Footer URL — same line you see on the website footer.
$g.DrawString('planmorph.software', $tinyFont, $mutedText, 16, 290)

$titleFont.Dispose(); $subFont.Dispose(); $tinyFont.Dispose()
$accentText.Dispose(); $fgText.Dispose(); $mutedText.Dispose()
$g.Dispose()

$bmp.Save((Join-Path $OutDir 'wizard.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()

# ── Small badge ────────────────────────────────────────────────────────
# 55×58 — sits in the top-right of every wizard page after the welcome
# screen. Same wordmark stacked tighter so it reads at thumbnail size.
$small = New-Object System.Drawing.Bitmap 55, 58
$g     = [System.Drawing.Graphics]::FromImage($small)
$g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

$bg = New-Object System.Drawing.SolidBrush $Surface
$g.FillRectangle($bg, 0, 0, 55, 58)
$bg.Dispose()

$accentBrush = New-Object System.Drawing.SolidBrush $Accent
$g.FillRectangle($accentBrush, 0, 0, 3, 58)
$accentBrush.Dispose()

$f1 = New-Object System.Drawing.Font 'Segoe UI Semibold', 11, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$f2 = New-Object System.Drawing.Font 'Segoe UI Semibold', 11, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$ab = New-Object System.Drawing.SolidBrush $AccentStrong
$fb = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(0xee, 0xee, 0xf0))
$g.DrawString('VEX',   $f1, $ab, 8, 12)
$g.DrawString('ATLAS', $f2, $fb, 8, 30)
$f1.Dispose(); $f2.Dispose(); $ab.Dispose(); $fb.Dispose()
$g.Dispose()

$small.Save((Join-Path $OutDir 'wizard-small.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$small.Dispose()

Write-Host "Wrote wizard.bmp + wizard-small.bmp into $OutDir"
