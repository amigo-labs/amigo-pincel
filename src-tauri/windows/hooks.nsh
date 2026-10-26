; Pincel's additions to Tauri's NSIS installer (bundle.windows.nsis
; .installerHooks), after amigo-blinzel's setup: Pincel registers as an
; application for its file types (Capabilities, RegisteredApplications,
; OpenWithProgids), so it shows up under Settings > Default apps and in
; "Open with". Windows lets the user, not the installer, pick defaults, so
; a first interactive install opens Default apps on Pincel's page.
; Silent (/S, winget) and passive (/P, the updater) installs don't.
; See docs/RELEASING.md and docs/specs/pincel.md §15, 2026-10-26.
;
; The type list mirrors bundle.fileAssociations in tauri.conf.json (the
; ProgID is each association's `name`); a test in src/main.rs keeps the
; two in step.

!define PINCEL_APP_KEY "Software\amigo-labs\Pincel"
!define PINCEL_CAPABILITIES "${PINCEL_APP_KEY}\Capabilities"

Var PincelFirstInstall

!macro PINCEL_REGISTER_TYPE EXT PROGID
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}\FileAssociations" ".${EXT}" "${PROGID}"
  WriteRegStr SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}" ""
!macroend

!macro PINCEL_UNREGISTER_TYPE EXT PROGID
  DeleteRegValue SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}"
!macroend

!macro PINCEL_TYPES OP
  !insertmacro ${OP} "aseprite" "Aseprite Sprite"
  !insertmacro ${OP} "ase" "Aseprite Sprite"
  !insertmacro ${OP} "ora" "OpenRaster Image"
  !insertmacro ${OP} "png" "PNG Image"
  !insertmacro ${OP} "jpg" "JPEG Image"
  !insertmacro ${OP} "jpeg" "JPEG Image"
  !insertmacro ${OP} "webp" "WebP Image"
  !insertmacro ${OP} "bmp" "BMP Image"
  !insertmacro ${OP} "gif" "GIF Image"
  !insertmacro ${OP} "tif" "TIFF Image"
  !insertmacro ${OP} "tiff" "TIFF Image"
!macroend

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $0 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${If} $0 == ""
    StrCpy $PincelFirstInstall 1
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationDescription" "Pixel-art and image editor"
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationIcon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  !insertmacro PINCEL_TYPES PINCEL_REGISTER_TYPE
  WriteRegStr SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}" "${PINCEL_CAPABILITIES}"
  ; SHCNE_ASSOCCHANGED: Explorer re-reads the associations.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  ${If} $PincelFirstInstall = 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    ExecShell "open" "ms-settings:defaultapps?registeredAppUser=${PRODUCTNAME}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; An update uninstalls the old version first; keep the registration.
  ${If} $UpdateMode <> 1
    !insertmacro PINCEL_TYPES PINCEL_UNREGISTER_TYPE
    DeleteRegValue SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}"
    DeleteRegKey SHCTX "${PINCEL_APP_KEY}"
    DeleteRegKey /ifempty SHCTX "Software\amigo-labs"
    System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ${EndIf}
!macroend
