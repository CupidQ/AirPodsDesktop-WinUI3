#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef PublishDir
  #define PublishDir "..\dist\publish"
#endif

[Setup]
AppId={{D7E9D1F6-E584-4A25-A52B-7D7B5ECA3F88}
AppName=AirPodsDesktop-WinUI3
AppVersion={#AppVersion}
AppPublisher=CupidQ
AppPublisherURL=https://github.com/CupidQ/AirPodsDesktop-WinUI3
AppSupportURL=https://github.com/CupidQ/AirPodsDesktop-WinUI3/issues
AppUpdatesURL=https://github.com/CupidQ/AirPodsDesktop-WinUI3/releases
DefaultDirName={localappdata}\Programs\AirPodsDesktop-WinUI3
DefaultGroupName=AirPodsDesktop-WinUI3
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.17763
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=AirPodsDesktop-WinUI3-{#AppVersion}-Setup-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\AirPodsDesktop.exe
VersionInfoVersion={#AppVersion}
VersionInfoProductName=AirPodsDesktop-WinUI3

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PublishDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\AirPodsDesktop-WinUI3"; Filename: "{app}\AirPodsDesktop.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\AirPodsDesktop-WinUI3"; Filename: "{app}\AirPodsDesktop.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\AirPodsDesktop.exe"; Description: "{cm:LaunchProgram,AirPodsDesktop-WinUI3}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  StartupCommand: String;
begin
  if CurUninstallStep = usUninstall then
    if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run',
        'AirPodsDesktop', StartupCommand) then
      if CompareText(StartupCommand, '"' + ExpandConstant('{app}\AirPodsDesktop.exe') + '"') = 0 then
        RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'AirPodsDesktop');
end;
