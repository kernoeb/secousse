; Suppress the post-install relaunch that tauri-plugin-updater forces via the `/R` flag.
;
; Why: the updater runs the installer with `/P /R` in passive mode, which makes
; `.onInstSuccess` call `RunAsUser` on the freshly-installed exe. Our flow installs
; on app close, so the user sees Secousse reopen itself right after they quit it.
;
; How: $PassiveMode is the only writable lever — `.onInstSuccess` gates the relaunch
; on `$PassiveMode = 1 || ${Silent}`. In `installMode: passive`, ${Silent} is false
; (NSIS sees `/P`, not `/S`), so clearing $PassiveMode here is enough to skip the
; RunAsUser call. We re-emit `SetAutoClose true` first because the stock template's
; auto-close block (which runs right after this hook) also gates on $PassiveMode.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $PassiveMode = 1
    SetAutoClose true
    StrCpy $PassiveMode 0
  ${EndIf}
!macroend
