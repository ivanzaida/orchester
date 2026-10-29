; Orchester's Windows installer (Inno Setup 6), built by .github/workflows/app-release.yml; see docs/stable-build.md,
; "Releases from CI". To build it by hand, after `cargo build --release --locked -p orchester-app -p orchester-runner`:
;
;   iscc /DAppVersion=<version> /DProgramDir=<folder with orchester.exe and orchester-runner.exe> /O<output folder>
;        tools\installer\orchester.iss
;
; The person's data. The app keeps its application database (orchester.db, with orchester.db-wal and orchester.db-shm),
; its data folder's lock (orchester.lock, orchester.lock.holder) and its logs (logs\) beside its program, in the install
; folder (orchester_store::location). This installer writes only orchester.exe, orchester-runner.exe and its own
; uninstaller there: an upgrade replaces those and leaves everything else, and the uninstaller removes only what was
; installed, so the data and the folder holding it stay. There is no [InstallDelete] or [UninstallDelete] section, and
; none may ever name the data.
;
; Per user, without administrator rights: the app writes its database beside itself, which a folder under Program Files
; does not allow, so there is no all-users install. The folder can be changed on the first install; an upgrade keeps it.
;
; A running Orchester is never closed or killed: while a program runs from the install folder, the install and the
; uninstall wait for the person to close it (Retry) or stop (Cancel).

#ifndef AppVersion
  #error Pass the app's version: iscc /DAppVersion=<version> /DProgramDir=<folder> tools\installer\orchester.iss
#endif
#ifndef ProgramDir
  #error Pass the folder of the built programs: iscc /DAppVersion=<version> /DProgramDir=<folder> tools\installer\orchester.iss
#endif
; Windows' file version takes numbers only, so a pre-release's suffix (0.3.0-rc.1) is left out there.
#if Pos("-", AppVersion) > 0
  #define NumericVersion Copy(AppVersion, 1, Pos("-", AppVersion) - 1)
#else
  #define NumericVersion AppVersion
#endif

[Setup]
; Fixed for good: Windows finds an installed Orchester to upgrade or uninstall by it.
AppId={{427D9341-C0D8-4294-A8E9-98C8C33B1703}
AppName=Orchester
AppVersion={#AppVersion}
AppVerName=Orchester {#AppVersion}
AppPublisher=Orchester
VersionInfoVersion={#NumericVersion}
VersionInfoProductVersion={#NumericVersion}
VersionInfoProductTextVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Orchester
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
; Inno's own closing of programs is off: the [Code] below asks the person to close Orchester instead.
CloseApplications=no
RestartApplications=no
SetupIconFile=..\..\crates\desktop-app\assets\icon\orchester.ico
UninstallDisplayIcon={app}\orchester.exe
UninstallDisplayName=Orchester
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
; The workflow passes /O; a build without it stays out of the source tree.
OutputDir=..\..\target\installer
OutputBaseFilename=Orchester-{#AppVersion}-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
english.AppRunning=Orchester is running from %1 (process %2). Close it, then choose Retry. Nothing has been changed.
english.DataKept=Your Orchester data in %1 was kept: the application database (orchester.db) and the logs. Delete that folder yourself if you no longer need them.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The app first: if it still runs although the check below let the install go on, its file is the first that cannot
; be replaced, before the runner is. Each program is named alone, never by a wildcard: the PDBs the release build
; writes beside them (orchester.pdb, orchester_runner.pdb) are GlitchTip's, to resolve error reports, and are never
; installed.
Source: "{#ProgramDir}\orchester.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProgramDir}\orchester-runner.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Orchester"; Filename: "{app}\orchester.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Orchester"; Filename: "{app}\orchester.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; The app registers its notifications' AppUserModelID itself on every start (crates/desktop-app/src/notifications);
; the uninstaller removes that registration and never creates it.
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\Orchester.App"; Flags: uninsdeletekey dontcreatekey

[Run]
Filename: "{app}\orchester.exe"; Description: "{cm:LaunchProgram,Orchester}"; Flags: nowait postinstall skipifsilent

[Code]
{ The processes of Orchester's programs running from Folder, as a list of their ids, or '' when none does: the app,
  its runner, and the app as builds before its rename named it (orchester-app.exe), which starts the same runner. }
function RunningFromFolder(const Folder: String): String;
var
  Locator, Service, Processes, Process: Variant;
  Index, Id: Integer;
begin
  Result := '';
  Locator := CreateOleObject('WbemScripting.SWbemLocator');
  Service := Locator.ConnectServer('.', 'root\CIMV2');
  Processes := Service.ExecQuery('SELECT ProcessId, ExecutablePath FROM Win32_Process WHERE Name = ''orchester.exe''' +
    ' OR Name = ''orchester-runner.exe'' OR Name = ''orchester-app.exe''');
  for Index := 0 to Processes.Count - 1 do
  begin
    Process := Processes.ItemIndex(Index);
    if not VarIsNull(Process.ExecutablePath) then
      if CompareText(RemoveBackslashUnlessRoot(ExtractFileDir(Process.ExecutablePath)),
        RemoveBackslashUnlessRoot(Folder)) = 0 then
      begin
        Id := Process.ProcessId;
        if Result <> '' then
          Result := Result + ', ';
        Result := Result + IntToStr(Id);
      end;
  end;
end;

{ RunningFromFolder, or '' when Windows cannot say (WMI unavailable). The check is then left to the file replacement,
  which cannot replace a running program and asks the person itself; the reason is in the setup log. }
function RunningFromFolderOrUnknown(const Folder: String): String;
begin
  try
    Result := RunningFromFolder(Folder);
  except
    Log('Could not check for a running Orchester: ' + GetExceptionMessage);
    Result := '';
  end;
end;

{ Waits until no Orchester program runs from Folder: '' then, or the message to stop with when the person cancels.
  A silent install cancels at once. }
function WaitUntilClosed(const Folder: String): String;
var
  Running: String;
begin
  Result := '';
  Running := RunningFromFolderOrUnknown(Folder);
  while Running <> '' do
  begin
    Result := FmtMessage(CustomMessage('AppRunning'), [Folder, Running]);
    if SuppressibleMsgBox(Result, mbError, MB_RETRYCANCEL, IDCANCEL) = IDCANCEL then
      Exit;
    Result := '';
    Running := RunningFromFolderOrUnknown(Folder);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := WaitUntilClosed(ExpandConstant('{app}'));
end;

function InitializeUninstall(): Boolean;
begin
  Result := WaitUntilClosed(ExpandConstant('{app}')) = '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Folder: String;
begin
  Folder := ExpandConstant('{app}');
  if (CurUninstallStep = usPostUninstall) and FileExists(Folder + '\orchester.db') then
    SuppressibleMsgBox(FmtMessage(CustomMessage('DataKept'), [Folder]), mbInformation, MB_OK, IDOK);
end;
