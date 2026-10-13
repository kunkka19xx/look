; The Windows installer for lookapp.exe. Per user, no elevation, the same
; layout and silent flags (/S, /D=) the Tauri bundler produced, so
; scripts/windows/install-look.ps1 keeps working.
;
;   makensis /DVERSION=1.2.3 /DBINARY=path\to\lookapp.exe /DOUTFILE=Look_1.2.3_x64-setup.exe look.nsi

!include "MUI2.nsh"

!ifndef VERSION
  !error "pass /DVERSION=x.y.z"
!endif
!ifndef BINARY
  !error "pass /DBINARY=path\to\lookapp.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "Look_${VERSION}_x64-setup.exe"
!endif

!define PRODUCT "Look"
!define EXE "lookapp.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
Unicode True
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\${PRODUCT}"
InstallDirRegKey HKCU "Software\${PRODUCT}" "InstallDir"
SetCompressor /SOLID lzma

!define MUI_ICON "..\..\apps\linows\icons\icon.ico"
!define MUI_UNICON "..\..\apps\linows\icons\icon.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  ; A running launcher holds its exe open.
  nsExec::ExecToLog 'taskkill /F /IM ${EXE}'
  SetOutPath "$INSTDIR"
  File "/oname=${EXE}" "${BINARY}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${EXE}"

  WriteRegStr HKCU "Software\${PRODUCT}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "kunkka19xx"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /F /IM ${EXE}'
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  ; The app registers its own autostart entry under this name.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCT}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  DeleteRegKey HKCU "Software\${PRODUCT}"
SectionEnd
