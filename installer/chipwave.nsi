; ChipWave Tracker - Windows x64 setup (NSIS 3, cross-buildable with makensis on Linux)
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

!define APPNAME "ChipWave Tracker"
!define PUBLISHER "postmanpat-303"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ChipWaveTracker"
!define ICON "..\assets\icon\chipwave_tracker.ico"

Name "${APPNAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\ChipWaveTracker" "InstallDir"
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
!define MUI_FINISHPAGE_RUN "$INSTDIR\ChipWave.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start ChipWave Tracker"

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
    MessageBox MB_ICONSTOP "ChipWave Tracker needs 64-bit Windows." /SD IDOK
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Section "ChipWave Tracker (required)" SecCore
  SectionIn RO
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  File "${PKGDIR}\ChipWave.exe"
  File "${PKGDIR}\README.txt"
  File "${PKGDIR}\LICENSE.txt"
  File "${PKGDIR}\THIRD_PARTY_LICENSES.txt"
  File "/oname=chipwave_tracker.ico" "${ICON}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\ChipWave.exe" "" "$INSTDIR\ChipWave.exe" 0
  CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\chipwave_tracker.ico" 0

  WriteRegStr HKLM "Software\ChipWaveTracker" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\ChipWave.exe,0"
  WriteRegStr HKLM "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTKEY}" "URLInfoAbout" "https://github.com/postmanpat-303/ChipWaveTracker"
  WriteRegStr HKLM "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\ChipWave.exe" "" "$INSTDIR\ChipWave.exe" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "The tracker, its licence and third-party notices, Start Menu shortcuts and the uninstaller."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Put a ChipWave Tracker shortcut on the desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  SetShellVarContext all
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"
  Delete "$INSTDIR\ChipWave.exe"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD_PARTY_LICENSES.txt"
  Delete "$INSTDIR\chipwave_tracker.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINSTKEY}"
  DeleteRegKey HKLM "Software\ChipWaveTracker"
  ; songs, settings and %APPDATA%\ChipWave recovery files are left alone
SectionEnd
