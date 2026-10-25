# Glossar

Begriffe, die in yAPPA auftauchen – vor allem in der Diagnose. Jeder Eintrag beginnt mit
einem Satz, der auch als Erklärung hinter dem ⓘ in der App erscheint. Wer es genauer wissen
will, findet darunter einen Link, meist zu Wikipedia.

## Latenz

Die Zeit, die dein Wort braucht, bis es beim anderen aus dem Kopfhörer kommt. Ziel in
yAPPA: 70 bis 120 Millisekunden von Mund zu Ohr.

Ab etwa 150 ms reden sich Leute ins Wort, weil die Pausen länger wirken, als sie sind. Die
Latenz setzt sich zusammen aus Mikrofonpuffer, Codec, Weg zum Server und zurück,
Jitterbuffer und Ausgabepuffer – jeder Teil ein paar Millisekunden.

Mehr dazu: [Latenz (Wikipedia)](https://de.wikipedia.org/wiki/Latenz_(Technik))

## Laufzeit zum Server

Wie lange ein Paket zum Server und wieder zurück braucht („Round Trip Time“, RTT). Unter
50 ms ist gut, über 150 ms merkt man es.

Die Laufzeit hängt fast nur von der Strecke und dem Anschluss ab, nicht von yAPPA. Per
WLAN kommen oft 10 bis 30 ms Schwankung dazu – ein Kabel ist die einfachste Verbesserung.

Mehr dazu: [Paketumlaufzeit (Wikipedia)](https://de.wikipedia.org/wiki/Paketumlaufzeit)

## Jitter

Wie unregelmäßig die Pakete ankommen. Große Schwankungen zwingen den Jitterbuffer, mehr zu
puffern – das kostet Latenz.

Jitter entsteht durch WLAN, volle Leitungen oder einen beschäftigten Router. Ein Wert unter
10 ms ist unauffällig, ab 30 ms ist er in der Diagnose orange.

Mehr dazu: [Jitter (Wikipedia)](https://de.wikipedia.org/wiki/Jitter)

## Jitterbuffer

Ein kleiner Vorrat an empfangenen Paketen, der Schwankungen ausgleicht, damit die Wiedergabe
nicht stockt. Je unruhiger das Netz, desto größer wird er – und desto mehr Latenz.

Der Jitterbuffer stellt sich selbst ein. Ein dauerhaft hoher Wert (über 120 ms) heißt: das
Netz ist unruhig, nicht die App.

Mehr dazu: [Jitterbuffer (Wikipedia)](https://de.wikipedia.org/wiki/Jitterbuffer)

## Paketverlust

Der Anteil der Pakete, die unterwegs verloren gehen. Bis etwa 2 % fängt der Codec das ab,
ab 5 % hört man Aussetzer oder eine Roboterstimme.

yAPPA schickt wichtige Teile doppelt (siehe RED), sodass einzelne verlorene Pakete nicht
auffallen. Paketverlust „Empfang“ betrifft, was bei dir ankommt; „Senden“ betrifft, was
von dir beim Server ankommt.

Mehr dazu: [Paketverlust (Wikipedia)](https://de.wikipedia.org/wiki/Paketverlust)

## Ersetzte Samples

Der Anteil der Tonstücke, die der Empfänger erfunden hat, weil das echte Paket fehlte oder
zu spät kam („Packet Loss Concealment“). Kurz unauffällig, dauerhaft über 2 % hörbar.

Der Decoder schätzt aus dem letzten Stück, wie es weitergeht. Das klappt bei einem
verlorenen Paket gut und bei mehreren hintereinander immer schlechter – daher die
Roboterstimme bei viel Paketverlust.

Mehr dazu: [Packet Loss Concealment (Wikipedia, englisch)](https://en.wikipedia.org/wiki/Packet_loss_concealment)

## Codec

Das Verfahren, das Sprache für den Versand klein macht und beim Empfänger wieder auspackt.
yAPPA benutzt Opus, den Standard für Sprache im Internet.

Opus klingt für Sprache ab etwa 32 kbit/s gut und ab 128 kbit/s kaum noch besser; es
verkraftet Paketverlust vergleichsweise gut. Welche Bitrate yAPPA nimmt, entscheidet die
**Sprachqualität**. In der Diagnose steht unter „Codec“, womit gerade gesendet wird.

Mehr dazu: [Opus (Wikipedia)](<https://de.wikipedia.org/wiki/Opus_(Audioformat)>)

## Sprachqualität

Wie viel Bandbreite deine Stimme bekommt – als Profil von „Sparsam“ (32 kbit/s) bis
„Stress“ (510 angefordert, etwa 256 erreicht). „Standard“ (80 kbit/s) ist der Wert, mit dem
die ersten Abende liefen; mehr ist Vergleich, nicht Alltag.

Hinter jedem Profil stehen vier Werte: die Bitrate von Opus, Mono oder Stereo, RED und DTX.
Im Profi-Modus unter „Netzwerk“ kannst du sie einzeln stellen; das Profil heißt dann „Eigen“.
Ein Wechsel im Call geht ohne Neubeitritt – die Stimme wird kurz neu gesendet, die anderen
hören eine kleine Pause. Mehr als 48 kHz Abtastrate gibt es nicht: Opus arbeitet intern mit
48 kHz, und oberhalb von 20 kHz hört kein Mensch etwas – ein 96-kHz-Regler würde nur
Rechenzeit kosten.

Mehr dazu: [Opus – Bitraten (Wikipedia)](<https://de.wikipedia.org/wiki/Opus_(Audioformat)#Eigenschaften>)

## Ausgehandelte Bitrate

Was der Encoder gerade wirklich anpeilt – gegen die Bitrate des Profils gehalten. Liegt sie
deutlich darunter, hat libwebrtc oder der Server die Rate gedrückt.

Die Zahl kommt aus den WebRTC-Statistiken des Senders („targetBitrate“). Sie ist orange,
wenn sie unter 80 % des Profils liegt – bei „Stress“ immer, weil libwebrtc Opus bei etwa
256 kbit/s deckelt. Daneben steht, ob ein oder zwei Kanäle gesendet werden.

Mehr dazu: [WebRTC Statistics (W3C, englisch)](https://www.w3.org/TR/webrtc-stats/#dom-rtcoutboundrtpstreamstats-targetbitrate)

## Senderate

Wie viele Kilobit pro Sekunde dein Mikrofon gerade an den Server schickt – mit allem drum
herum: Opus-Daten, RED-Kopien und Paketköpfe. Die Nutzlast in Klammern ist ohne die Köpfe.

Bei „Standard“ sind das rund 160 bis 180 kbit/s (80 für Opus, 80 für die RED-Kopie, der
Rest Paketköpfe), bei „Stress“ etwa 270 – mehr lässt libwebrtc nicht zu. Selbst das fällt
neben einem Spiel oder Stream kaum ins Gewicht; ein knapper Upload merkt es trotzdem.

Mehr dazu: [Bitrate (Wikipedia)](https://de.wikipedia.org/wiki/Bitrate)

## Stereo

Zwei Kanäle statt einem. In yAPPA liegt auf beiden dasselbe Mikrofonsignal – das misst, was
Stereo an Bandbreite und Rechenzeit kostet, es klingt nicht räumlicher.

Opus kodiert zwei gleiche Kanäle sehr effizient (die Differenz ist null), darum ist der
Unterschied zu Mono bei gleicher Bitrate klein. Ein echtes Stereo-Mikrofon ist nicht
gebaut; Stimme ist Mono. Beim Empfänger wird Stereo auf einen Kanal gemischt.

Mehr dazu: [Stereofonie (Wikipedia)](https://de.wikipedia.org/wiki/Stereofonie)

## DTX

„Discontinuous Transmission“: in Sprechpausen werden fast keine Pakete geschickt. Spart
Bandbreite, kostet beim Einsetzen der Stimme manchmal die erste Silbe.

In yAPPA ist DTX in allen Profilen aus, weil die Sprachaktivierung die Pause ohnehin
stumm schaltet und ein stetiger Paketstrom den Jitterbuffer ruhig hält.

Mehr dazu: [Discontinuous Transmission (Wikipedia, englisch)](https://en.wikipedia.org/wiki/Discontinuous_transmission)

## FEC

„Forward Error Correction“ in Opus selbst: jedes Paket trägt eine grobe Version des vorigen
mit. Fehlt ein Paket, klingt die Stelle dumpfer statt leer. libwebrtc hat das immer an.

Im Unterschied zu RED (eine volle Kopie) ist die FEC-Version kleiner und schlechter. Beides
zusammen hilft bei vereinzelten Verlusten; die Einstellung ist in yAPPA nicht wählbar.

Mehr dazu: [Vorwärtsfehlerkorrektur (Wikipedia)](https://de.wikipedia.org/wiki/Vorwärtsfehlerkorrektur)

## Rahmenlänge

Wie viel Ton ein Opus-Paket enthält: 20 Millisekunden in libwebrtc. Kürzer (10 ms) hieße
weniger Latenz, aber doppelt so viele Pakete und Paketköpfe.

yAPPA reicht dem Encoder 10-ms-Blöcke; libwebrtc fasst je zwei zu einem Paket zusammen.
Die Rahmenlänge ist im SDK nicht einstellbar.

Mehr dazu: [Opus – Rahmengrößen (Wikipedia)](<https://de.wikipedia.org/wiki/Opus_(Audioformat)>)

## Komplexität

Wie viel Rechenzeit Opus beim Kodieren investiert, 0 bis 10. libwebrtc nimmt 9; Stufe 10
klingt bei gleicher Bitrate minimal besser und kostet spürbar mehr CPU.

Die Einstellung ist im LiveKit-SDK nicht erreichbar; der Wert steht hier, damit klar ist,
womit gemessen wurde.

Mehr dazu: [Opus – Encoder (Wikipedia)](<https://de.wikipedia.org/wiki/Opus_(Audioformat)>)

## RED

„Redundant Encoding“: jedes Paket enthält zusätzlich eine Kopie des vorherigen. Geht eines
verloren, steckt sein Inhalt im nächsten. Kostet etwas Bandbreite, rettet viele Aussetzer.

RED ist der Grund, warum die Senderate höher ist als die reine Opus-Bitrate. Es hilft bei
vereinzelten Verlusten; bei einer kaputten Leitung hilft es nicht. Über etwa 200 kbit/s
passt die Kopie nicht mehr ins Paket (1 kB), dann lässt libwebrtc sie stillschweigend weg.

Mehr dazu: [RFC 2198 (englisch)](https://www.rfc-editor.org/rfc/rfc2198)

## Abtastrate

Wie oft pro Sekunde der Ton gemessen wird. yAPPA arbeitet mit 48 000 Hz; ein Gerät mit
anderer Rate wird umgerechnet.

Opus arbeitet intern mit 48 kHz, und oberhalb von 20 kHz hört kein Mensch etwas – mehr
Abtastrate (96 kHz) brächte für die Sprachübertragung nichts, nur Rechenzeit. Darum gibt es
dafür keinen Regler.

Mehr dazu: [Abtastrate (Wikipedia)](https://de.wikipedia.org/wiki/Abtastrate)

## Puffer

Die Portion Ton, die die Soundkarte auf einmal liefert oder abholt – angegeben in Frames und
Millisekunden. Kleiner Puffer: weniger Latenz, aber eher Aussetzer.

yAPPA verarbeitet den Ton in Stücken von 10 ms; welche Portion das Gerät liefert, steht im
Diagnose. Manche Treiber geben nur 20 oder 30 ms – das ist in Ordnung, nur eben etwas
mehr Latenz.

Mehr dazu: [Puffer (Wikipedia)](<https://de.wikipedia.org/wiki/Puffer_(Informatik)>)

## Aussetzer

Die Soundkarte wollte Ton, und keiner lag bereit (oder umgekehrt: Ton kam, und keiner holte
ihn ab). Jeder Aussetzer ist ein Knacken oder ein kurzes Loch.

Ursachen sind meist ein überlasteter Rechner, ein USB-Gerät an einem schwachen Anschluss
oder ein Treiber im Energiesparmodus. Ein paar beim Start sind normal; wenn sie während des
Gesprächs weiterzählen, bitte melden.

Mehr dazu: [Pufferunterlauf (Wikipedia)](https://de.wikipedia.org/wiki/Pufferunterlauf)

## Pegel

Wie laut ein Signal ist, in Dezibel relativ zum Maximum (dBFS). 0 dB ist das lauteste
Mögliche, –60 dB ist fast Stille. Beim Sprechen sind –20 bis –10 dB ein guter Bereich.

Der Balken in yAPPA zeigt den Pegel nach dem Mikrofon, also das, was die anderen hören. Mit
dem Regler bei einer Person änderst du nur, wie laut sie bei dir ankommt.

Mehr dazu: [dBFS (Wikipedia)](https://de.wikipedia.org/wiki/DBFS)

## Lautstärke-Angleich

Bringt jede Stimme bei dir auf etwa dieselbe Lautstärke: Wer leise ankommt, wird langsam
angehoben, wer laut ankommt, abgesenkt – höchstens 12 dB, ein paar dB pro Sekunde. Pausen
und Rauschen werden nicht hochgezogen.

Der Regler bei einer Person wirkt obendrauf. Im Profi-Modus lässt sich der Angleich unter
„Audio und Sprechen“ abschalten; die Diagnose zeigt, wie viel er bei wem gerade nachregelt.

Mehr dazu: [Automatic Gain Control (Wikipedia)](https://de.wikipedia.org/wiki/Automatic_Gain_Control)

## Server

Der Rechner, über den die Sprache aller läuft. Er mischt nicht, sondern verteilt: jede
Stimme kommt einzeln bei dir an, und du stellst sie selbst ein.

yAPPA benutzt dafür LiveKit, einen selbst gehosteten Medienserver, der bei einem von uns zu
Hause oder auf einem Mietserver läuft. So bleibt alles in unserer Hand – keine Firma hört
mit.

Mehr dazu: [Selective Forwarding Unit (Wikipedia, englisch)](https://en.wikipedia.org/wiki/Selective_forwarding_unit)

## Einladung

Ein Link, der mit `yappa://invite/` beginnt und genau einmal funktioniert: Er sagt yAPPA,
welcher Server gemeint ist, und lässt dich einmal hinein. Danach kennt yAPPA dich auf diesem
Rechner.

Im Link stehen ein einmaliger Code und die Adresse der Verwaltung des Servers. Beim Einlösen
bekommst du einen eigenen Zugang, den yAPPA in der Windows-Anmeldeinformationsverwaltung
(macOS: Schlüsselbund) ablegt. Für Entwickler gibt es noch die alte Form `yappa1.…`, die
direkt in einen Raum führt.

## Echo

Wenn jemand Lautsprecher statt Kopfhörer benutzt, nimmt sein Mikrofon die anderen mit auf –
und alle hören sich selbst mit Verzögerung. Eine Echounterdrückung gibt es in yAPPA noch
nicht.

Deshalb: Headset. Die Echounterdrückung („AEC“) steht auf der Liste, ist aber schwierig, ohne
Sprachqualität zu verlieren.

Mehr dazu: [Echokompensation (Wikipedia)](https://de.wikipedia.org/wiki/Echokompensation)

## Push-to-Talk

Das Mikrofon ist nur offen, solange eine Taste gedrückt ist. In yAPPA ist das der Modus
„Taste halten“ unter „Audio und Sprechen“; die Taste wählst du dort oder unter „Tasten“.

Die Taste ist global: sie funktioniert auch, wenn ein Spiel im Vollbild läuft, und das
Spiel bekommt sie trotzdem. Die Alternative ist die Sprachaktivierung mit einstellbarer
Schwelle, die Voreinstellung.

Mehr dazu: [Push-to-talk (Wikipedia)](https://de.wikipedia.org/wiki/Push-to-talk)

## Stumm und Taub

Stumm heißt: dein Mikrofon ist aus, du hörst aber alle. Taub heißt: du hörst nichts –
und sendest auch nichts, denn wer nichts hört, will nicht ins Leere reden.

Beides schaltest du im Dock unten links oder mit einer globalen Taste aus „Tasten“ – die
wirkt auch mitten im Spiel. Die anderen sehen „stumm“ oder „taub“ hinter deinem Namen.
Hebst du Taub wieder auf, ist dein Mikrofon so wie davor: an, wenn es an war, aus, wenn du
es vorher selbst ausgeschaltet hattest.

## Rauschunterdrückung

Entfernt Lüfter, Tastatur und Straßenlärm aus dem Mikrofonsignal, bevor es gesendet wird.
Noch nicht in yAPPA – kommt mit der DSP-Kette.

Vorgesehen ist DeepFilterNet, ein neuronales Netz, das in Echtzeit auf der CPU läuft. Es
soll sich abschalten lassen, falls es die Stimme verfärbt.

Mehr dazu: [Rauschunterdrückung (Wikipedia)](https://de.wikipedia.org/wiki/Rauschunterdrückung)
