; WO7 release-only NSIS hooks. Never include in the default unsigned smoke config.
; All executable assets and policy are staged into resources/ by the reviewed
; signed payload builder. Per-machine elevation is required by the Tauri config.
!macro NSIS_HOOK_PREINSTALL
  UserInfo::GetAccountType
  Pop $0
  StrCmp $0 "Admin" +3
    MessageBox MB_ICONSTOP "Lu IDE protected installation requires Administrator approval."
    Abort "WO7: per-machine elevation absent"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$INSTDIR\resources\installer\wo7-service-lifecycle.ps1" +3
    MessageBox MB_ICONSTOP "Lu IDE signed service lifecycle helper missing."
    Abort "WO7: service lifecycle helper missing"
  IfFileExists "$INSTDIR\resources\wo7-service-policy.json" +3
    MessageBox MB_ICONSTOP "Lu IDE signed service policy missing."
    Abort "WO7: service policy missing"
  ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\resources\installer\wo7-service-lifecycle.ps1" -Operation Install -InstallRoot "$INSTDIR"' $0
  StrCmp $0 "0" wo7_service_install_complete
    ; A partial service cannot be left silently installed on failed setup.
    ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\resources\installer\wo7-service-lifecycle.ps1" -Operation Remove -InstallRoot "$INSTDIR"' $1
    MessageBox MB_ICONSTOP "Lu IDE protected recovery service could not be safely installed."
    Abort "WO7: protected service admission failed"
  wo7_service_install_complete:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\resources\installer\wo7-service-lifecycle.ps1" +3
    MessageBox MB_ICONSTOP "Cannot verify protected service removal helper."
    Abort "WO7: uninstall service helper missing"
  ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\resources\installer\wo7-service-lifecycle.ps1" -Operation Remove -InstallRoot "$INSTDIR"' $0
  StrCmp $0 "0" wo7_service_uninstall_complete
    MessageBox MB_ICONSTOP "Refusing incomplete uninstall while protected recovery service may remain."
    Abort "WO7: protected service cleanup not verified"
  wo7_service_uninstall_complete:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; No cleanup guesses: PREUNINSTALL already required observed SCM removal.
!macroend
