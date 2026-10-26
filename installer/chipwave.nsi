; ChipWave Studio (formerly ChipWave Tracker) - Windows x64 setup (NSIS 3, cross-buildable with makensis on Linux)
; Build: makensis -DVERSION=0.2.2.2 -DPKGDIR=../dist/pkg -DOUTFILE=../dist/ChipWave-0.2.2.2-setup-x64.exe installer/chipwave.nsi
; Silent install: ChipWave-<ver>-setup-x64.exe /S   (desktop shortcut included by default)
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef PKGDIR
  !define PKGDIR "..\dist\pkg"
!endif
!ifndef OUTFILE
  !define OUTFILE "..\dist\ChipWave-${VERSION}-setup-x64.exe"
!endif

!define APPNAME "ChipWave Studio"
!define PUBLISHER "Del (postmanpat)"
; The install folder is "ChipWave Studio" now. It was "ChipWave Tracker" up to 0.2.3.19 and kept
; that name for a while afterwards so upgrades landed in the same place: the setup now moves an old
; default install out of the old folder, and leaves a junction at the old path so tools that still
; look for "ChipWave Tracker\ChipWave.exe" (Wave Hub's published versions) keep working.
; The registry and uninstall keys keep their old names: one Add/Remove Programs entry, and Wave Hub
; finds the install through them.
!define INSTFOLDER "ChipWave Studio"
!define OLDFOLDER "ChipWave Tracker"
!define REGKEY "Software\ChipWaveTracker"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ChipWaveTracker"
!define ICON "..\assets\icon\chipwave_studio.ico"
; ChipWave Tracker's shortcuts, removed when installing over it
!define OLDNAME "ChipWave Tracker"

Name "${APPNAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${INSTFOLDER}"
InstallDirRegKey HKLM "${REGKEY}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "${APPNAME} ${VERSION}"

!ifndef VIVERSION
  !define VIVERSION "${VERSION}"
!endif
VIProductVersion "${VIVERSION}"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APPNAME} setup"
VIAddVersionKey "LegalCopyright" "GPL-3.0-or-later"
VIAddVersionKey "CompanyName" "${PUBLISHER}"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Start ChipWave Studio"
; Start it as the signed-in user, not elevated like this setup: an elevated process can't see
; the user's mapped network drives (Z: etc. belong to the non-elevated logon session), so a
; ChipWave started from here showed no network drives. Explorer starts it unelevated.
!define MUI_FINISHPAGE_RUN_FUNCTION StartUnelevated

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PKGDIR}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "ChipWave Studio needs 64-bit Windows." /SD IDOK
    Abort
  ${EndIf}
  SetRegView 64
  ; InstallDirRegKey ran before SetRegView 64, so read the 64-bit key again.
  ; If the previous install sits in the old "ChipWave Tracker" folder, move to the new name;
  ; a custom folder the user picked is respected.
  ReadRegStr $0 HKLM "${REGKEY}" "InstallDir"
  ${If} $0 != ""
    ${If} $0 == "$PROGRAMFILES64\${OLDFOLDER}"
      StrCpy $INSTDIR "$PROGRAMFILES64\${INSTFOLDER}"
    ${Else}
      StrCpy $INSTDIR $0
    ${EndIf}
  ${EndIf}
FunctionEnd

; Delete the old "ChipWave Tracker" install once the new folder is written, so there is only one
; copy of the app. Skipped when the user chose to install there anyway.
Function RemoveOldFolder
  StrCpy $0 "$PROGRAMFILES64\${OLDFOLDER}"
  ${If} $0 == "$INSTDIR"
    Return
  ${EndIf}
  ${IfNot} ${FileExists} "$0\ChipWave.exe"
    Return
  ${EndIf}
  Delete "$0\ChipWave.exe"
  Delete "$0\README.txt"
  Delete "$0\LICENSE.txt"
  Delete "$0\THIRD_PARTY_LICENSES.txt"
  Delete "$0\chipwave_studio.ico"
  Delete "$0\chipwave_tracker.ico"
  Delete "$0\Uninstall.exe"
  RMDir "$0"
FunctionEnd

; Leave a directory junction at the old path pointing at the new folder, so anything that still
; looks for "ChipWave Tracker\ChipWave.exe" (Wave Hub's published versions) keeps finding the app.
; Best effort: a failure here changes nothing for the user.
Function CreateCompatJunction
  StrCpy $0 "$PROGRAMFILES64\${OLDFOLDER}"
  ${If} $0 == "$INSTDIR"
    Return
  ${EndIf}
  ${If} ${FileExists} "$0\*.*"
    Return
  ${EndIf}
  nsExec::ExecToLog 'cmd /c mklink /J "$0" "$INSTDIR"'
  Pop $0
FunctionEnd

Function StartUnelevated
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\ChipWave.exe"'
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

; Remove ChipWave Tracker's shortcuts (0.2.3.19 and older) and its old icon file.
; Songs, settings and %APPDATA%\ChipWave are shared with ChipWave Studio and stay.
Function RemoveOldShortcuts
  SetShellVarContext all
  Delete "$DESKTOP\${OLDNAME}.lnk"
  Delete "$SMPROGRAMS\${OLDNAME}\${OLDNAME}.lnk"
  Delete "$SMPROGRAMS\${OLDNAME}\Uninstall ${OLDNAME}.lnk"
  RMDir "$SMPROGRAMS\${OLDNAME}"
  Delete "$INSTDIR\chipwave_tracker.ico"
  Delete "$PROGRAMFILES64\${OLDFOLDER}\chipwave_tracker.ico"
FunctionEnd

Section "ChipWave Studio (required)" SecCore
  SectionIn RO
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  File "${PKGDIR}\ChipWave.exe"
  File "${PKGDIR}\README.txt"
  File "${PKGDIR}\LICENSE.txt"
  File "${PKGDIR}\THIRD_PARTY_LICENSES.txt"
  File "/oname=chipwave_studio.ico" "${ICON}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\ChipWave.exe" "" "$INSTDIR\ChipWave.exe" 0
  CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\chipwave_studio.ico" 0

  WriteRegStr HKLM "${REGKEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\ChipWave.exe,0"
  WriteRegStr HKLM "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTKEY}" "URLInfoAbout" "https://github.com/postmanpat-303/ChipWaveStudio"
  WriteRegStr HKLM "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoRepair" 1
  Call RemoveOldShortcuts
  Call RemoveOldFolder
  Call CreateCompatJunction
SectionEnd

Section "Desktop shortcut" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\ChipWave.exe" "" "$INSTDIR\ChipWave.exe" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "The tracker, its licence and third-party notices, Start Menu shortcuts and the uninstaller."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Put a ChipWave Studio shortcut on the desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  SetShellVarContext all
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"
  Delete "$DESKTOP\${OLDNAME}.lnk"
  Delete "$SMPROGRAMS\${OLDNAME}\${OLDNAME}.lnk"
  Delete "$SMPROGRAMS\${OLDNAME}\Uninstall ${OLDNAME}.lnk"
  RMDir "$SMPROGRAMS\${OLDNAME}"
  Delete "$INSTDIR\ChipWave.exe"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD_PARTY_LICENSES.txt"
  Delete "$INSTDIR\chipwave_studio.ico"
  Delete "$INSTDIR\chipwave_tracker.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ; the junction at the old name, if this install left one
  ${If} "$PROGRAMFILES64\${OLDFOLDER}" != "$INSTDIR"
    RMDir "$PROGRAMFILES64\${OLDFOLDER}"
  ${EndIf}
  DeleteRegKey HKLM "${UNINSTKEY}"
  DeleteRegKey HKLM "${REGKEY}"
  ; songs, settings and %APPDATA%\ChipWave recovery files are left alone
SectionEnd
