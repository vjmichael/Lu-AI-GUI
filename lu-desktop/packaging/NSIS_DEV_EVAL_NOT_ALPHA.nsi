; Lu independent Tauri/React development evaluation installer.
; NOT a release recipe: RESOURCE_COMPILER_VERIFIED must be validated before alpha.
Unicode true
!ifndef LU_DESKTOP_EXE
  !error "Provide -DLU_DESKTOP_EXE=<absolute path to cross-built desktop.exe>"
!endif
!ifndef LU_ENGINE_DIR
  !error "Provide -DLU_ENGINE_DIR=<absolute path to pinned engine directory>"
!endif
!ifndef LU_OUTFILE
  !error "Provide -DLU_OUTFILE=<absolute path for evaluation installer>"
!endif
!ifndef LU_README
  !error "Provide -DLU_README=<absolute path to evaluation README>"
!endif
Name "Lu R15 Independent Desktop - Development Evaluation (NOT ALPHA — COMMON CONTROLS V6 FIX)"
OutFile "${LU_OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\LuR15DesktopDevEval"
RequestExecutionLevel user
SetCompressor zlib
ShowInstDetails show
ShowUninstDetails show
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Lu Desktop Development Evaluation" SEC_APP
  SetOutPath "$INSTDIR"
  File /oname=lu-desktop.exe "${LU_DESKTOP_EXE}"
  File /oname=README_NOT_ALPHA.txt "${LU_README}"
  File "${LU_ENGINE_DIR}/msvcp140.dll"
  File "${LU_ENGINE_DIR}/vcruntime140.dll"
  File "${LU_ENGINE_DIR}/vcruntime140_1.dll"
  SetOutPath "$INSTDIR\lu-inference"
  File "${LU_ENGINE_DIR}/llama-server.exe"
  File "${LU_ENGINE_DIR}/manifest.json"
  File "${LU_ENGINE_DIR}/msvcp140.dll"
  File "${LU_ENGINE_DIR}/vcruntime140.dll"
  File "${LU_ENGINE_DIR}/vcruntime140_1.dll"
  SetOutPath "$INSTDIR"
  CreateDirectory "$SMPROGRAMS\Lu R15 Desktop Dev Eval"
  CreateShortcut "$SMPROGRAMS\Lu R15 Desktop Dev Eval\Lu R15 Desktop Dev Eval.lnk" "$INSTDIR\lu-desktop.exe"
  CreateShortcut "$SMPROGRAMS\Lu R15 Desktop Dev Eval\Read Me (NOT Alpha).lnk" "$INSTDIR\README_NOT_ALPHA.txt"
  WriteUninstaller "$INSTDIR\UninstallLuR15DevEval.exe"
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\Lu R15 Desktop Dev Eval\Lu R15 Desktop Dev Eval.lnk"
  Delete "$SMPROGRAMS\Lu R15 Desktop Dev Eval\Read Me (NOT Alpha).lnk"
  RMDir "$SMPROGRAMS\Lu R15 Desktop Dev Eval"
  Delete "$INSTDIR\UninstallLuR15DevEval.exe"
  Delete "$INSTDIR\lu-desktop.exe"
  Delete "$INSTDIR\README_NOT_ALPHA.txt"
  Delete "$INSTDIR\msvcp140.dll"
  Delete "$INSTDIR\vcruntime140.dll"
  Delete "$INSTDIR\vcruntime140_1.dll"
  Delete "$INSTDIR\lu-inference\manifest.json"
  Delete "$INSTDIR\lu-inference\llama-server.exe"
  Delete "$INSTDIR\lu-inference\msvcp140.dll"
  Delete "$INSTDIR\lu-inference\vcruntime140.dll"
  Delete "$INSTDIR\lu-inference\vcruntime140_1.dll"
  RMDir "$INSTDIR\lu-inference"
  RMDir "$INSTDIR"
  ; This intentionally leaves user-owned Lu chat, GGUF and logs in AppData.
SectionEnd
