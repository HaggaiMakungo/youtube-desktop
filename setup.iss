; YouTube Desktop Installer
; Built with Inno Setup 6
; This script turns the app binary into a proper Windows installer that doesn't look like garbage
; 
; To build: npm run tauri:build && npm run dist
; Output: ./installer/YouTube-Desktop-Setup.exe
;
#define MyAppName      "YouTube Desktop"
#define MyAppVersion   "0.1.0"
#define MyAppPublisher "GH0STHUNT3R"
#define MyAppExeName   "youtube-desktop.exe"
#define MyAppRoot      "C:\Users\GH0STHUNT3R\Desktop\Dev\Youtube"
#define SignToolPath   "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
#define PfxFile        "{#MyAppRoot}\signing\youtube-desktop.pfx"

[Setup]
; Main installer configuration — tell Inno Setup how to not fuck this up
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL=https://github.com/GH0STHUNT3R
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir={#MyAppRoot}\installer
OutputBaseFilename=YouTube-Desktop-Setup
SetupIconFile={#MyAppRoot}\src-tauri\icons\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
DisableDirPage=yes

; Note: signing is handled as separate signtool calls before/after this build,
; not via Inno Setup's built-in SignTool integration (see build instructions).

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
; Copy the binary and icon so people actually have something to run
; Binary should already be signed; if not, the build script fucked up
Source: "{#MyAppRoot}\src-tauri\target\release\{#MyAppExeName}"; \
    DestDir: "{app}"; \
    Flags: ignoreversion

; App icon (used for shortcuts)
Source: "{#MyAppRoot}\src-tauri\icons\icon.ico"; \
    DestDir: "{app}"; \
    Flags: ignoreversion

[Icons]
; Start Menu
Name: "{group}\{#MyAppName}";          Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"

; Desktop (optional — only if user ticked the task above)
Name: "{autodesktop}\{#MyAppName}";    Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Registry]
; Shove the auto-start key into HKCU because apparently people want this launching every boot
; Only runs for the current user (HKCU), not all users — thank god
Root: HKCU; \
    Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; \
    ValueName: "{#MyAppName}"; \
    ValueData: """{app}\{#MyAppExeName}"""; \
    Flags: uninsdeletevalue

[Run]
; Ask if they want to launch it (most people click yes, then forget they did)
Filename: "{app}\{#MyAppExeName}"; \
    Description: "Launch {#MyAppName}"; \
    Flags: nowait postinstall skipifsilent

[UninstallRun]
; Murder the process before uninstalling or Windows will cry about locked files
Filename: "taskkill.exe"; \
    Parameters: "/f /im {#MyAppExeName}"; \
    Flags: runhidden; \
    RunOnceId: "KillYouTubeDesktop"

[Code]
// Signing is handled externally via signtool before/after this build.
