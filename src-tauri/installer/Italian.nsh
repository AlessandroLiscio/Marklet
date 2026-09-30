; The Italian strings for the NSIS installer, overriding Tauri's own.
;
; Tauri ships an Italian translation, and three of its lines are broken. The
; installer's "the app is running" dialog does not interpolate the product name
; the way a template does — `utils.nsh` inside tauri-bundler performs a literal
; runtime `StrReplace` of the literal string `{{product_name}}`:
;
;   nsis_tauri_utils::StrReplace "$(appRunning)" "{{product_name}}" "${productName}"
;
; The Italian file writes the FIRST occurrence in each string with two braces
; and the second with one — `{product_name}`, and in one case `{product_name}}`.
; StrReplace never matches those, so they reach the dialog exactly as written.
; Reinstalling with Marklet open showed "Chiudi {product_name} e riprova." The
; English file has two braces throughout, which is why nobody saw it there.
;
; Checked against tauri-cli 2.11.4 and still present on `dev` at the time of
; writing. This file is the same translation with the braces corrected; when
; upstream fixes it, delete this file and the `customLanguageFiles` entry in
; `tauri.conf.json` rather than keeping a fork of a translation we do not own.
;
; A custom language file must be COMPLETE: the bundler writes it verbatim in
; place of Tauri's, so a string left out here is a string the installer has no
; text for. Every LangString from Tauri's own Italian.nsh is present below.

LangString addOrReinstall ${LANG_ITALIAN} "Aggiungi/reinstalla componenti"
LangString alreadyInstalled ${LANG_ITALIAN} "Programma già installato"
LangString alreadyInstalledLong ${LANG_ITALIAN} "${PRODUCTNAME} ${VERSION} è già installato.$\nPer continuare scegli l'operazione da eseguire e seleziona 'Avanti'."
LangString appRunning ${LANG_ITALIAN} "{{product_name}} è in esecuzione!$\nChiudi {{product_name}} e riprova."
LangString appRunningOkKill ${LANG_ITALIAN} "{{product_name}} è in esecuzione!$\nPer chiudere {{product_name}} seleziona 'OK'"
LangString chooseMaintenanceOption ${LANG_ITALIAN} "Scegli l'operazione di manutenzione da eseguire."
LangString choowHowToInstall ${LANG_ITALIAN} "Scegli come vuoi installare ${PRODUCTNAME}."
LangString createDesktop ${LANG_ITALIAN} "Crea collegamento sul desktop"
LangString dontUninstall ${LANG_ITALIAN} "Non disinstallare"
LangString dontUninstallDowngrade ${LANG_ITALIAN} "Non disinstallare (per questo installer il downgrade senza la disinstallazione è disabilitato)"
LangString failedToKillApp ${LANG_ITALIAN} "Impossibile chiudere {{product_name}}.$\nChiudi {{product_name}} e poi riprova"
LangString installingWebview2 ${LANG_ITALIAN} "Installazione WebView2..."
LangString newerVersionInstalled ${LANG_ITALIAN} "È già installata una versione più recente di ${PRODUCTNAME}!$\nNon è consigliato installare una versione più vecchia.$\nSe vuoi comunque procedere, è meglio prima disinstallare la versione attuale.$\nPer continuare scegli l'operazione da eseguire e seleziona 'Avanti'."
LangString older ${LANG_ITALIAN} "più vecchia"
LangString olderOrUnknownVersionInstalled ${LANG_ITALIAN} "Nel sistema è installata una versione $R4 di ${PRODUCTNAME}.$\nPrima di procedere all'installazione è consigliabile disinstallare la versione attuale.$\nPer continuare scegli l'operazione da eseguire e seleziona 'Avanti'."
LangString silentDowngrades ${LANG_ITALIAN} "Per questo installer i downgrade sono disabilitati , impossibile procedere con l'installer silenzioso, usa invece l'installer con interfaccia grafica.$\n"
LangString unableToUninstall ${LANG_ITALIAN} "Impossibile disinstallare!"
LangString uninstallApp ${LANG_ITALIAN} "Disinstalla ${PRODUCTNAME}"
LangString uninstallBeforeInstalling ${LANG_ITALIAN} "Disinstalla prima di installare"
LangString unknown ${LANG_ITALIAN} "sconosciuta"
LangString webview2AbortError ${LANG_ITALIAN} "Errore nell'installazione di WebView2!$\nL'app non può funzionare senza.$\nProva a riavviare l'installer."
LangString webview2DownloadError ${LANG_ITALIAN} "Errore: il download di WebView2 è fallito - $0"
LangString webview2DownloadSuccess ${LANG_ITALIAN} "Download bootstrapper WebView2 completato"
LangString webview2Downloading ${LANG_ITALIAN} "Download bootstrapper WebView2..."
LangString webview2InstallError ${LANG_ITALIAN} "Errore: l'installazione di WebView2 è fallita con codice errore $1"
LangString webview2InstallSuccess ${LANG_ITALIAN} "WebView2 installato correttamente"
LangString deleteAppData ${LANG_ITALIAN} "Cancella i dati dell'applicazione"
