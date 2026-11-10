; Lu current-source Lu engineering preview installer.
; CURRENT SOURCE, CPU + Vulkan source-built backends, unsigned. Not historical alpha, not production..
Unicode true
!ifndef LU_APP_EXE
  !error "Set LU_APP_EXE to the exact-head compiled Windows desktop"
!endif
!ifndef LU_ENGINE_EXE
  !error "Set LU_ENGINE_EXE to source-built, checksum-pinned llama-server.exe"
!endif
!ifndef LU_VULKAN_ENGINE_EXE
  !error "Set LU_VULKAN_ENGINE_EXE to separately built source-pinned Vulkan server"
!endif
!ifndef LU_ENGINE_MANIFEST
  !error "Set LU_ENGINE_MANIFEST to the engine manifest"
!endif
!ifndef LU_TEST_README
  !error "Set LU_TEST_README to the human test checklist"
!endif
!ifndef LU_CODECS_DIR
  !error "Set LU_CODECS_DIR to the host-verified native codecs directory"
!endif
!ifndef LU_BUILD_PROVENANCE
  !error "Set LU_BUILD_PROVENANCE to exact-commit build provenance"
!endif
!ifndef LU_INSTALLER_OUTPUT
  !error "Set LU_INSTALLER_OUTPUT to the output executable"
!endif

Name "Lu Current Development Preview (Unsigned)"
OutFile "${LU_INSTALLER_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\LuCurrentPreview"
RequestExecutionLevel user
SetCompressor lzma
ShowInstDetails show
ShowUninstDetails show
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Lu Current-source Windows development preview" SEC_TEST
  SetOutPath "$INSTDIR"
  File /oname=lu-desktop.exe "${LU_APP_EXE}"
  File /oname=README_CURRENT_PREVIEW.txt "${LU_TEST_README}"
  File /oname=BUILD_PROVENANCE.json "${LU_BUILD_PROVENANCE}"
  SetOutPath "$INSTDIR\lu-inference"
  File /oname=llama-server.exe "${LU_ENGINE_EXE}"
  File /oname=llama-server-vulkan.exe "${LU_VULKAN_ENGINE_EXE}"
  File /oname=manifest.json "${LU_ENGINE_MANIFEST}"
  SetOutPath "$INSTDIR\lu-native-codecs"
  File /r "${LU_CODECS_DIR}\*.*"
!ifdef LU_BROWSER_DIR
  ; Only a reviewed content-pinned Chromium/Node/Playwright runtime may be
  ; packaged. The native host re-verifies the embedded complete manifest
  ; before any process launch or network authorization.
  SetOutPath "$INSTDIR\lu-browser"
  File /r "${LU_BROWSER_DIR}\*.*"
!endif
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\UninstallLuCurrentPreview.exe"
  CreateDirectory "$SMPROGRAMS\Lu Current Preview"
  CreateShortcut "$SMPROGRAMS\Lu Current Preview\Lu Current Preview.lnk" "$INSTDIR\lu-desktop.exe"
  CreateShortcut "$SMPROGRAMS\Lu Current Preview\Read Me - Test Only.lnk" "$INSTDIR\README_CURRENT_PREVIEW.txt"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview" "DisplayName" "Lu Current Development Preview (Unsigned)"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview" "UninstallString" '"$INSTDIR\UninstallLuCurrentPreview.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview" "NoRepair" 1
SectionEnd

Section "Uninstall"
  ; Test app binaries only: never delete %APPDATA%, chats, GGUF or journals.
  Delete "$SMPROGRAMS\Lu Current Preview\Lu Current Preview.lnk"
  Delete "$SMPROGRAMS\Lu Current Preview\Read Me - Test Only.lnk"
  RMDir "$SMPROGRAMS\Lu Current Preview"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LuCurrentPreview"
  Delete "$INSTDIR\UninstallLuCurrentPreview.exe"
  Delete "$INSTDIR\lu-desktop.exe"
  Delete "$INSTDIR\README_CURRENT_PREVIEW.txt"
  Delete "$INSTDIR\BUILD_PROVENANCE.json"
  Delete "$INSTDIR\lu-inference\llama-server.exe"
  Delete "$INSTDIR\lu-inference\llama-server-vulkan.exe"
  Delete "$INSTDIR\lu-inference\manifest.json"
  RMDir "$INSTDIR\lu-inference"
  RMDir /r "$INSTDIR\lu-native-codecs"
  ; App-owned binary runtime only. Never touch model store, AppData or chats.
  RMDir /r "$INSTDIR\lu-browser"
  RMDir "$INSTDIR"
SectionEnd
