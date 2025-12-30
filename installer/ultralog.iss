; UltraLog Windows Installer Script
; Inno Setup Script for UltraLog - ECU Log Viewer
; https://github.com/SomethingNew71/UltraLog

#define MyAppName "UltraLog"
#define MyAppVersion GetEnv('ULTRALOG_VERSION')
#define MyAppPublisher "Cole Gentry"
#define MyAppURL "https://github.com/SomethingNew71/UltraLog"
#define MyAppExeName "ultralog.exe"
#define MyAppId "{{8F4E0E7D-3C5A-4B2D-9E1F-6A7B8C9D0E1F}"

[Setup]
; Application identity
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases

; Installation settings
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes

; Output settings
OutputDir=..\output
OutputBaseFilename=ultralog-windows-setup
SetupIconFile=..\assets\icons\windows.ico
UninstallDisplayIcon={app}\{#MyAppExeName}

; Compression
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes

; Privileges - don't require admin for per-user install
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

; Visual settings
WizardStyle=modern
DisableWelcomePage=no

; Version info embedded in setup.exe
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup
VersionInfoCopyright=Copyright (c) 2025 {#MyAppPublisher}
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}

; Code signing placeholders (uncomment when certificate is available)
; SignTool=signtool sign /f "$CERTIFICATE_PATH$" /p "$CERTIFICATE_PASSWORD$" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $f
; SignedUninstaller=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "fileassoc"; Description: "Associate with ECU log files (.csv, .log, .mlg, .xrk, .drk, .llg)"; GroupDescription: "File associations:"; Flags: unchecked

[Files]
; Main executable
Source: "..\target\x86_64-pc-windows-msvc\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Start Menu
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
; Desktop (optional)
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; File associations (optional, only if user selects the task)
; CSV files
Root: HKCU; Subkey: "Software\Classes\.csv\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
; LOG files
Root: HKCU; Subkey: "Software\Classes\.log\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
; MLG files (Speeduino/rusEFI)
Root: HKCU; Subkey: "Software\Classes\.mlg"; ValueType: string; ValueName: ""; ValueData: "UltraLog.LogFile"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\.mlg\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
; XRK files (AiM)
Root: HKCU; Subkey: "Software\Classes\.xrk"; ValueType: string; ValueName: ""; ValueData: "UltraLog.LogFile"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\.xrk\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
; DRK files (AiM)
Root: HKCU; Subkey: "Software\Classes\.drk"; ValueType: string; ValueName: ""; ValueData: "UltraLog.LogFile"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\.drk\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
; LLG files (Link ECU)
Root: HKCU; Subkey: "Software\Classes\.llg"; ValueType: string; ValueName: ""; ValueData: "UltraLog.LogFile"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\.llg\OpenWithProgids"; ValueType: string; ValueName: "UltraLog.LogFile"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc

; ProgID registration
Root: HKCU; Subkey: "Software\Classes\UltraLog.LogFile"; ValueType: string; ValueName: ""; ValueData: "ECU Log File"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\UltraLog.LogFile\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\UltraLog.LogFile\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: fileassoc

; App registration for "Open with" menu
Root: HKCU; Subkey: "Software\Classes\Applications\{#MyAppExeName}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#MyAppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

[Run]
; Option to launch after install
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Pascal Script for silent update mode detection
var
  SilentUpdate: Boolean;

function InitializeSetup(): Boolean;
begin
  // Check if this is a silent update (auto-updater)
  SilentUpdate := (Pos('/VERYSILENT', UpperCase(GetCmdTail)) > 0) or
                  (Pos('/SILENT', UpperCase(GetCmdTail)) > 0);
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  // Notify shell of file association changes
  if CurStep = ssPostInstall then
  begin
    // Refresh shell icons/associations
    RegWriteStringValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Explorer', 'Refresh', '1');
  end;
end;

function InitializeUninstall(): Boolean;
begin
  Result := True;
end;
