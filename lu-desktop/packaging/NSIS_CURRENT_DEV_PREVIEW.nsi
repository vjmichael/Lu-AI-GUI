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
!ifndef LU_RESEARCH_DIR
  !error "Set LU_RESEARCH_DIR to verified signed Windows HTTPS client"
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

!ifdef LU_RELEASE_V1
  !define LU_NAME "Lu v1.0"
  !define LU_DIRECTORY "Lu"
  !define LU_UNINSTALL "UninstallLu.exe"
  !define LU_MENU "Lu"
  !define LU_REGKEY "Lu"
  !define LU_README_NAME "README_LU_V1.txt"
  !define LU_RELEASE_VERSION "1.0.0"
!else
  !define LU_NAME "Lu Current Development Preview (Unsigned)"
  !define LU_DIRECTORY "LuCurrentPreview"
  !define LU_UNINSTALL "UninstallLuCurrentPreview.exe"
  !define LU_MENU "Lu Current Preview"
  !define LU_REGKEY "LuCurrentPreview"
  !define LU_README_NAME "README_CURRENT_PREVIEW.txt"
  !define LU_RELEASE_VERSION "0.2.0-rc.1"
!endif
Name "${LU_NAME}"
OutFile "${LU_INSTALLER_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\${LU_DIRECTORY}"
RequestExecutionLevel user
SetCompressor lzma
ShowInstDetails show
ShowUninstDetails show
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "${LU_NAME}" SEC_TEST
  SetOutPath "$INSTDIR"
  File /oname=lu-desktop.exe "${LU_APP_EXE}"
  File /oname=${LU_README_NAME} "${LU_TEST_README}"
  File /oname=BUILD_PROVENANCE.json "${LU_BUILD_PROVENANCE}"
  SetOutPath "$INSTDIR\lu-inference"
  File /oname=llama-server.exe "${LU_ENGINE_EXE}"
  File /oname=llama-server-vulkan.exe "${LU_VULKAN_ENGINE_EXE}"
  File /oname=manifest.json "${LU_ENGINE_MANIFEST}"
  SetOutPath "$INSTDIR\lu-native-codecs"
  File /r "${LU_CODECS_DIR}\*.*"
  SetOutPath "$INSTDIR\lu-research"
  File /oname=curl.exe "${LU_RESEARCH_DIR}\curl.exe"
!ifdef LU_BROWSER_DIR
  ; Only a reviewed content-pinned Chromium/Node/Playwright runtime may be
  ; packaged. The native host re-verifies the embedded complete manifest
  ; before any process launch or network authorization.
  SetOutPath "$INSTDIR\lu-browser"
  File /r "${LU_BROWSER_DIR}\*.*"
!endif
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\${LU_UNINSTALL}"
  CreateDirectory "$SMPROGRAMS\${LU_MENU}"
  CreateShortcut "$SMPROGRAMS\${LU_MENU}\${LU_MENU}.lnk" "$INSTDIR\lu-desktop.exe"
  CreateShortcut "$SMPROGRAMS\${LU_MENU}\Read Me.lnk" "$INSTDIR\${LU_README_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "DisplayName" "${LU_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "DisplayVersion" "${LU_RELEASE_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "UninstallString" '"$INSTDIR\${LU_UNINSTALL}"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  ; Test app binaries only: never delete %APPDATA%, chats, GGUF or journals.
  Delete "$SMPROGRAMS\${LU_MENU}\${LU_MENU}.lnk"
  Delete "$SMPROGRAMS\${LU_MENU}\Read Me.lnk"
  RMDir "$SMPROGRAMS\${LU_MENU}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LU_REGKEY}"
  Delete "$INSTDIR\${LU_UNINSTALL}"
  Delete "$INSTDIR\lu-desktop.exe"
  Delete "$INSTDIR\${LU_README_NAME}"
  Delete "$INSTDIR\BUILD_PROVENANCE.json"
  Delete "$INSTDIR\lu-inference\llama-server.exe"
  Delete "$INSTDIR\lu-inference\llama-server-vulkan.exe"
  Delete "$INSTDIR\lu-inference\manifest.json"
  RMDir "$INSTDIR\lu-inference"
  RMDir /r "$INSTDIR\lu-native-codecs"
  Delete "$INSTDIR\lu-research\curl.exe"
  RMDir "$INSTDIR\lu-research"
  ; App-owned binary runtime only. Never touch model store, AppData or chats.
  RMDir /r "$INSTDIR\lu-browser"
  RMDir "$INSTDIR"
SectionEnd
