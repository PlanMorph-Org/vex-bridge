#ifndef AppVersion
#define AppVersion "dev"
#endif

#ifndef SourceDir
#define SourceDir "dist\vex-bridge-dev-windows-x86_64"
#endif

#ifndef OutputDir
#define OutputDir "dist"
#endif

; The Microsoft Edge WebView2 Evergreen Bootstrapper: a small (~2 MB),
; Microsoft-signed stub that installs/updates the shared WebView2 Runtime the
; desktop app's webview depends on. It is NOT the multi-hundred-MB standalone
; runtime — only the official bootstrapper, exactly as documented at
; https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution#online-only-deployment
;
; The release pipeline (.github/workflows/release.yml) downloads this file
; straight from Microsoft's official redistribution point into `SourceDir`
; before invoking ISCC, the same way it already stages the `vex` engine
; binary. Nothing in this script downloads or executes unverified content:
; if the file isn't present at compile time the installer simply omits
; WebView2 provisioning (see `HasWebView2Bootstrapper` below) rather than
; failing or reaching out to the network itself.
#ifndef WebView2BootstrapperFile
#define WebView2BootstrapperFile "MicrosoftEdgeWebView2Setup.exe"
#endif

#if FileExists(SourceDir + "\" + WebView2BootstrapperFile)
  #define HasWebView2Bootstrapper
#else
  #pragma message "WebView2 bootstrapper not found in SourceDir (" + SourceDir + "\" + WebView2BootstrapperFile + "); the setup executable will not provision the WebView2 Runtime. See release.yml for how the release pipeline stages it."
#endif

[Setup]
AppId={{18C92BE2-59E8-48C5-9B58-18F66B5C56D4}
AppName=Vex Atlas
AppVersion={#AppVersion}
AppVerName=Vex Atlas {#AppVersion}
AppPublisher=PlanMorph
AppPublisherURL=https://studio.planmorph.software
AppSupportURL=https://studio.planmorph.software/install
DefaultDirName={localappdata}\Programs\Vex Atlas
DefaultGroupName=Vex Atlas
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=VexAtlasSetup-{#AppVersion}-windows-x86_64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
UninstallDisplayIcon={app}\vex-desktop.exe
; Stop a running Vex Atlas before overwriting its binaries. Without this,
; Windows refuses to replace a locked .exe, leaving a stale daemon running next
; to freshly-copied binaries (version skew) — the root cause of the
; "uninstall and reinstall" ritual. We additionally force-stop the processes in
; PrepareToInstall below, because the daemon/tray run windowless and the built-in
; "applications in use" detector only catches windowed apps reliably.
CloseApplications=yes
CloseApplicationsFilter=vex-bridge.exe,vex-tray.exe,vex-desktop.exe,vex.exe
RestartApplications=no

[Tasks]
Name: "autostart"; Description: "Start Vex Atlas tray when I sign in"; GroupDescription: "Startup:"; Flags: checkedonce
Name: "desktopicon"; Description: "Create a Vex Atlas desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\vex.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vex-bridge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vex-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vex-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\SHA256SUMS"; DestDir: "{app}"; Flags: ignoreversion
#ifdef HasWebView2Bootstrapper
Source: "{#SourceDir}\{#WebView2BootstrapperFile}"; DestDir: "{app}"; Flags: ignoreversion deleteafterinstall
#endif

[Icons]
Name: "{group}\Vex Atlas"; Filename: "{app}\vex-desktop.exe"
Name: "{group}\Vex Atlas Tray"; Filename: "{app}\vex-tray.exe"
Name: "{group}\Pair Vex Atlas Device"; Filename: "{app}\vex-desktop.exe"
Name: "{autodesktop}\Vex Atlas"; Filename: "{app}\vex-desktop.exe"; Tasks: desktopicon
Name: "{userstartup}\Vex Atlas Tray"; Filename: "{app}\vex-tray.exe"; Tasks: autostart

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{code:UserPathWithApp}"; Check: NeedsPathEntry; Flags: preservestringtype

[Run]
#ifdef HasWebView2Bootstrapper
; Provision the shared WebView2 Runtime before Vex Atlas ever tries to open
; its window, but only if it isn't already present (the vast majority of
; Windows 10/11 machines already have it via Microsoft Edge). Runs during
; silent/unattended installs too — this is a required dependency, not an
; optional app launch — but only when NeedsWebView2Runtime finds no usable
; install; "waituntilterminated" keeps install order deterministic (runtime
; ready before the tray/desktop [Run] entries below launch).
Filename: "{app}\{#WebView2BootstrapperFile}"; Parameters: "/silent /install"; StatusMsg: "Installing Microsoft Edge WebView2 Runtime..."; Check: NeedsWebView2Runtime; Flags: waituntilterminated
#endif
Filename: "{app}\vex-tray.exe"; Description: "Start Vex Atlas tray"; Flags: nowait skipifsilent
Filename: "{app}\vex-desktop.exe"; Description: "Open Vex Atlas"; Flags: nowait skipifsilent

[Code]
const
  VEX_HWND_BROADCAST = $FFFF;
  VEX_WM_SETTINGCHANGE = $001A;
  VEX_SMTO_ABORTIFHUNG = $0002;
  // The Evergreen WebView2 Runtime's stable EdgeUpdate client GUID, per
  // Microsoft's documented detection method:
  // https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution#detect-if-a-webview2-runtime-is-already-installed
  VEX_WEBVIEW2_CLIENT_GUID = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function SendMessageTimeout(hWnd: Longint; Msg: Longint; wParam: Longint; lParam: String;
  fuFlags: Longint; uTimeout: Longint; var lpdwResult: Longint): Longint;
  external 'SendMessageTimeoutW@user32.dll stdcall';

function ExistingUserPath(): String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Result) then
    Result := '';
end;

function PathContainsApp(PathValue: String): Boolean;
begin
  Result := Pos(';' + Uppercase(ExpandConstant('{app}')) + ';', ';' + Uppercase(PathValue) + ';') > 0;
end;

function NeedsPathEntry(): Boolean;
begin
  Result := not PathContainsApp(ExistingUserPath());
end;

function UserPathWithApp(Param: String): String;
var
  Current: String;
begin
  Current := ExistingUserPath();
  if Current = '' then
    Result := ExpandConstant('{app}')
  else if PathContainsApp(Current) then
    Result := Current
  else
    Result := Current + ';' + ExpandConstant('{app}');
end;

procedure BroadcastEnvironmentChange();
var
  ResultCode: Longint;
begin
  SendMessageTimeout(VEX_HWND_BROADCAST, VEX_WM_SETTINGCHANGE, 0, 'Environment',
    VEX_SMTO_ABORTIFHUNG, 5000, ResultCode);
end;

// `pv` is unset/empty/"0.0.0.0" for a client key that exists but has no
// runtime actually installed (e.g. left over from an uninstall) — treat
// only a real version string as "installed".
function IsUsableWebView2Version(Version: String): Boolean;
begin
  Result := (Version <> '') and (Version <> '0.0.0.0');
end;

function WebView2VersionAtKey(const RootKey: Integer; const SubKeyPrefix: String): String;
var
  Value: String;
begin
  Result := '';
  if RegQueryStringValue(RootKey, SubKeyPrefix + '\Microsoft\EdgeUpdate\Clients\' +
    VEX_WEBVIEW2_CLIENT_GUID, 'pv', Value) then
    Result := Value;
end;

// Detect an already-usable WebView2 Runtime the same way Microsoft's own
// distribution guidance recommends: a per-machine install (WOW6432Node on
// 64-bit Windows, or the native SOFTWARE key on 32-bit Windows) OR a
// per-user install both count, since this installer runs at
// PrivilegesRequired=lowest and the Evergreen bootstrapper falls back to a
// per-user install when it can't elevate.
function IsWebView2RuntimeInstalled(): Boolean;
begin
  Result :=
    IsUsableWebView2Version(WebView2VersionAtKey(HKEY_LOCAL_MACHINE, 'SOFTWARE\WOW6432Node')) or
    IsUsableWebView2Version(WebView2VersionAtKey(HKEY_LOCAL_MACHINE, 'SOFTWARE')) or
    IsUsableWebView2Version(WebView2VersionAtKey(HKEY_CURRENT_USER, 'SOFTWARE'));
end;

// [Run] `Check:` callback for the bundled WebView2 bootstrapper entry.
function NeedsWebView2Runtime(): Boolean;
begin
  Result := not IsWebView2RuntimeInstalled();
end;

procedure RemoveAppFromUserPath();
var
  Current: String;
  AppDir: String;
begin
  Current := ExistingUserPath();
  AppDir := ExpandConstant('{app}');
  StringChangeEx(Current, AppDir + ';', '', True);
  StringChangeEx(Current, ';' + AppDir, '', True);
  if CompareText(Current, AppDir) = 0 then
    Current := '';
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Current);
  BroadcastEnvironmentChange();
end;

procedure StopVexProcesses();
var
  ResultCode: Integer;
begin
  // Force-stop every Vex Atlas process before we touch the binaries. The daemon
  // and tray run windowless, so Inno's built-in in-use detection is unreliable
  // for them; taskkill /T also reaps any child `vex.exe` the daemon spawned.
  // /F is required because the daemon ignores WM_CLOSE (it has no message loop).
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM vex-desktop.exe', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM vex-tray.exe', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM vex-bridge.exe', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM vex.exe', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
  // Give Windows a moment to release the file handles before the copy step.
  Sleep(800);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopVexProcesses();
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    BroadcastEnvironmentChange();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    StopVexProcesses()
  else if CurUninstallStep = usPostUninstall then
    RemoveAppFromUserPath();
end;