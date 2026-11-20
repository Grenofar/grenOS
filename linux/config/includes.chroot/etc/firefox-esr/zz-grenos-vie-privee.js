// Ce que Firefox envoie à Mozilla, coupé (20 novembre).
//
// Grenofar, après l'inventaire de ce qui part sur le réseau : « coupe-le ».
// Firefox est le seul programme de l'image qui parle de lui-même à son
// éditeur : mesures d'usage, études, page d'accueil avec tuiles
// SPONSORISÉES (Temu, adidas… vues dans sa VM le 19), articles Pocket, météo,
// suggestions sponsorisées dans la barre d'adresse.
//
// Lu par le lien `syspref` du paquet (usr/share/firefox-esr/browser/defaults/
// syspref -> /etc/firefox-esr), comme zz-grenos-video.js ; la construction
// vérifie ce lien. `pref` et non `lockPref` : ce sont des valeurs par défaut,
// chacun peut en rallumer une dans les paramètres de Firefox.
//
// GARDÉ, exprès : la protection contre les sites dangereux (Safe Browsing)
// et les listes de blocage et de révocation de certificats (Remote Settings).
// Elles protègent la personne ; les couper serait une perte de sécurité.

// Mesures d'usage et rapports.
pref("datareporting.healthreport.uploadEnabled", false);
pref("datareporting.policy.dataSubmissionEnabled", false);
pref("datareporting.usage.uploadEnabled", false);
pref("toolkit.telemetry.enabled", false);
pref("toolkit.telemetry.unified", false);
pref("toolkit.telemetry.archive.enabled", false);
pref("toolkit.telemetry.newProfilePing.enabled", false);
pref("toolkit.telemetry.shutdownPingSender.enabled", false);
pref("toolkit.telemetry.firstShutdownPing.enabled", false);
pref("toolkit.telemetry.bhrPing.enabled", false);
pref("browser.ping-centre.telemetry", false);
pref("browser.newtabpage.activity-stream.feeds.telemetry", false);
pref("browser.newtabpage.activity-stream.telemetry", false);
pref("dom.private-attribution.submission.enabled", false);

// Rapports de plantage envoyés.
pref("breakpad.reportURL", "");
pref("browser.tabs.crashReporting.sendReport", false);
pref("browser.crashReports.unsubmittedCheck.autoSubmit2", false);

// Études et expériences poussées par Mozilla.
pref("app.shield.optoutstudies.enabled", false);
pref("app.normandy.enabled", false);
pref("app.normandy.api_url", "");

// La page d'accueil : ni sponsors, ni articles, ni météo.
pref("browser.newtabpage.activity-stream.showSponsored", false);
pref("browser.newtabpage.activity-stream.showSponsoredTopSites", false);
pref("browser.newtabpage.activity-stream.feeds.section.topstories", false);
pref("browser.newtabpage.activity-stream.feeds.topsites", true);
pref("browser.newtabpage.activity-stream.showWeather", false);
pref("browser.newtabpage.activity-stream.system.showWeather", false);
pref("extensions.pocket.enabled", false);

// La barre d'adresse : pas de suggestions de Mozilla, ni sponsorisées.
pref("browser.urlbar.suggest.quicksuggest.sponsored", false);
pref("browser.urlbar.suggest.quicksuggest.nonsponsored", false);
pref("browser.urlbar.quicksuggest.dataCollection.enabled", false);

// Recommandations d'extensions et de fonctions.
pref("browser.discovery.enabled", false);
pref("extensions.htmlaboutaddons.recommendations.enabled", false);
pref("extensions.getAddons.showPane", false);
pref("browser.newtabpage.activity-stream.asrouter.userprefs.cfr.addons", false);
pref("browser.newtabpage.activity-stream.asrouter.userprefs.cfr.features", false);

// La vérification « suis-je derrière un portail Wi-Fi ? », qui appelle
// detectportal.firefox.com à chaque démarrage et changement de réseau.
// NetworkManager sait déjà dire si la connexion passe.
pref("network.captive-portal-service.enabled", false);
pref("network.connectivity-service.enabled", false);
