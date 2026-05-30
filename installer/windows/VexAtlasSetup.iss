#ifndef AppVersion
#define AppVersion "dev"
#endif

#ifndef SourceDir
#define SourceDir "dist\vex-bridge-dev-windows-x86_64"
#endif

#ifndef OutputDir
#define OutputDir "dist"
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
UninstallDisplayIcon={app}\vex-tray.exe

[Tasks]
Name: "autostart"; Description: "Start Vex Atlas tray when I sign in"; GroupDescription: "Startup:"; Flags: checkedonce

[Files]
Source: "{#SourceDir}\vex.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vex-bridge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vex-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\SHA256SUMS"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Vex Atlas Tray"; Filename: "{app}\vex-tray.exe"
Name: "{group}\Pair Vex Atlas Device"; Filename: "{app}\vex-bridge.exe"; Parameters: "pair --open-browser"
Name: "{userstartup}\Vex Atlas Tray"; Filename: "{app}\vex-tray.exe"; Tasks: autostart

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{code:UserPathWithApp}"; Check: NeedsPathEntry; Flags: preservestringtype

[Run]
Filename: "{app}\vex-tray.exe"; Description: "Start Vex Atlas tray"; Flags: nowait skipifsilent
Filename: "{app}\vex-bridge.exe"; Parameters: "pair --open-browser"; Description: "Pair this device in your browser"; Flags: nowait runhidden skipifsilent

[Code]
const
  HWND_BROADCAST = $FFFF;
  WM_SETTINGCHANGE = $001A;
  SMTO_ABORTIFHUNG = $0002;

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
  SendMessageTimeout(HWND_BROADCAST, WM_SETTINGCHANGE, 0, 'Environment',
    SMTO_ABORTIFHUNG, 5000, ResultCode);
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

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    BroadcastEnvironmentChange();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveAppFromUserPath();
end;