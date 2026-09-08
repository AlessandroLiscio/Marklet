; NSIS hooks for the Marklet installer.
;
; These deliberately contain NO registry key paths. `src-tauri/src/platform/windows.rs`
; owns the key list and is the single source of truth; the installer shells out to the
; binary so the two can never drift. Two copies of a registry key list always drift, and
; the symptom is orphaned keys nobody notices for months.
;
; Everything is written under HKCU: no elevation, per-user install, clean uninstall.
;
; Phase P6 (platform-engineer) implements --install / --uninstall behind these calls.
; See .claude/skills/win-integration/SKILL.md.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering Markdown file association..."
  nsExec::ExecToLog '"$INSTDIR\marklet.exe" --install --silent'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing Markdown file association..."
  nsExec::ExecToLog '"$INSTDIR\marklet.exe" --uninstall --silent'
!macroend
