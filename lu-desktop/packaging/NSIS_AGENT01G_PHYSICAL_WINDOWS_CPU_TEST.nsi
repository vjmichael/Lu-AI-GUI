; Lu AGENT-01G physical Windows engineering acceptance installer.
; TEST ONLY, CPU-only, unsigned. Never alpha or production.
Unicode true
!ifndef LU_APP_EXE
  !error "Set LU_APP_EXE to the exact-head compiled Windows desktop"
!endif
!ifndef LU_ENGINE_EXE
  !error "Set LU_ENGINE_EXE to source-built, checksum-pinned llama-server.exe"
!endif
!ifndef LU_ENGINE_MANIFEST
  !error "Set LU_ENGINE_MANIFEST to the engine manifest"
!endif
!ifndef LU_TEST_README
  !error "Set LU_TEST_README to the human test checklist"
!endif
!ifndef LU_BUILD_PROVENANCE
  !error "Set LU_BUILD_PROVENANCE to exact-commit build provenance"
!endif
!ifndef LU_INSTALLER_OUTPUT
  !error "Set LU_INSTALLER_OUTPUT to the output executable"
!endif

Name "Lu AGENT-01G Physical Windows CPU Test (NOT ALPHA)"
OutFile "${LU_INSTALLER_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\LuAGENT01GPhysicalTest"
RequestExecutionLevel user
SetCompressor lzma
ShowInstDetails show
ShowUninstDetails show
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Lu AGENT-01G physical engineering evaluation" SEC_TEST
  SetOutPath "$INSTDIR"
  File /oname=lu-desktop.exe "${LU_APP_EXE}"
  File /oname=README_PHYSICAL_WINDOWS_NOT_ALPHA.txt "${LU_TEST_README}"
  File /oname=BUILD_PROVENANCE.json "${LU_BUILD_PROVENANCE}"
  SetOutPath "$INSTDIR\lu-inference"
  File /oname=llama-server.exe "${LU_ENGINE_EXE}"
  File /oname=manifest.json "${LU_ENGINE_MANIFEST}"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\UninstallLuAGENT01GPhysicalTest.exe"
  CreateDirectory "$SMPROGRAMS\Lu AGENT-01G Physical Test"
  CreateShortcut "$SMPROGRAMS\Lu AGENT-01G Physical Test\Lu AGENT-01G Physical Test.lnk" "$INSTDIR\lu-desktop.exe"
  CreateShortcut "$SMPROGRAMS\Lu AGENT-01G Physical Test\Read Me - Test Only.lnk" "$INSTDIR\README_PHYSICAL_WINDOWS_NOT_ALPHA.txt"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest" "DisplayName" "Lu AGENT-01G Physical Windows CPU Test (NOT ALPHA)"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest" "UninstallString" '"$INSTDIR\UninstallLuAGENT01GPhysicalTest.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest" "NoRepair" 1
SectionEnd

Section "Uninstall"
  ; Test app binaries only: never delete %APPDATA%, chats, GGUF or journals.
  Delete "$SMPROGRAMS\Lu AGENT-01G Physical Test\Lu AGENT-01G Physical Test.lnk"
  Delete "$SMPROGRAMS\Lu AGENT-01G Physical Test\Read Me - Test Only.lnk"
  RMDir "$SMPROGRAMS\Lu AGENT-01G Physical Test"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuAGENT01GPhysicalTest"
  Delete "$INSTDIR\UninstallLuAGENT01GPhysicalTest.exe"
  Delete "$INSTDIR\lu-desktop.exe"
  Delete "$INSTDIR\README_PHYSICAL_WINDOWS_NOT_ALPHA.txt"
  Delete "$INSTDIR\BUILD_PROVENANCE.json"
  Delete "$INSTDIR\lu-inference\llama-server.exe"
  Delete "$INSTDIR\lu-inference\manifest.json"
  RMDir "$INSTDIR\lu-inference"
  RMDir "$INSTDIR"
SectionEnd
