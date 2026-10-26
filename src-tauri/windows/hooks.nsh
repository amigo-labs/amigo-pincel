; Pincel's additions to Tauri's NSIS installer (bundle.windows.nsis
; .installerHooks), after amigo-blinzel's setup: Pincel registers as an
; application for its file types (Capabilities, RegisteredApplications,
; OpenWithProgids), so it shows up under Settings > Default apps and in
; "Open with". Windows lets the user, not the installer, pick defaults, so
; the first interactive install opens Default apps on Pincel's page, once:
; the value DefaultAppsOffered under Tauri's product key remembers it
; across upgrades, including the interactive "uninstall first" path, which
; deletes the uninstall key. Only "delete app data" on uninstall clears it.
; Silent (/S, winget) and passive (/P, the updater) installs never open it.
; See docs/RELEASING.md and docs/specs/pincel.md §15, 2026-10-26.
;
; The type list mirrors bundle.fileAssociations in tauri.conf.json (the
; ProgID is each association's `name`); a test in src/main.rs keeps the
; two in step.

; Under Tauri's own key (Software\<publisher>\<product>, which holds the
; install location).
!define PINCEL_CAPABILITIES "${MANUPRODUCTKEY}\Capabilities"

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

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationDescription" "Pixel-art and image editor"
  WriteRegStr SHCTX "${PINCEL_CAPABILITIES}" "ApplicationIcon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  !insertmacro PINCEL_TYPES PINCEL_REGISTER_TYPE
  WriteRegStr SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}" "${PINCEL_CAPABILITIES}"
  ; SHCNE_ASSOCCHANGED: Explorer re-reads the associations.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  ReadRegStr $0 SHCTX "${MANUPRODUCTKEY}" "DefaultAppsOffered"
  ${If} $0 == ""
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    ExecShell "open" "ms-settings:defaultapps?registeredAppUser=${PRODUCTNAME}"
    WriteRegStr SHCTX "${MANUPRODUCTKEY}" "DefaultAppsOffered" "1"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; An update uninstalls the old version first; keep the registration.
  ${If} $UpdateMode <> 1
    !insertmacro PINCEL_TYPES PINCEL_UNREGISTER_TYPE
    DeleteRegValue SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}"
    DeleteRegKey SHCTX "${PINCEL_CAPABILITIES}"
    System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ${EndIf}
!macroend
