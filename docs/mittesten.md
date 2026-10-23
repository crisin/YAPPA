# Mittesten

Für alle aus der Gang, die yAPPA ausprobieren. Du brauchst Windows 10 oder 11, ein Headset
und zwei Dinge von Chris: den **Installer** und deinen persönlichen **Einladungslink** (er
beginnt mit `yappa://invite/`).

## Loslegen

1. Installer starten. Windows warnt vor einer unbekannten App: „Weitere Informationen" →
   „Trotzdem ausführen". Das liegt daran, dass die App noch nicht signiert ist.
2. Auf den **Einladungslink** klicken. yAPPA öffnet sich und hat den Link schon eingefügt.
   Klappt der Klick nicht (manche Chat-Programme öffnen nur Web-Links), den Link kopieren,
   yAPPA öffnen und ihn ins Feld **Einladung** einfügen.
3. Deinen **Namen** eintragen – so sehen dich die anderen – und auf **Beitreten** klicken.
4. **Hört dich yAPPA?** Sag etwas: Der Balken muss sich bewegen. Wenn nicht, oben ein
   anderes Mikrofon wählen. Darunter Kopfhörer oder Headset als Ausgabe. **Weiter.**
5. **Wie redest du?** Headset oder Lautsprecher wählen; yAPPA stellt passend ein, wann
   dein Mikro angeht. Bei „Taste halten“ einmal auf **Taste festlegen** und die Taste
   drücken (gut: eine Seitentaste der Maus). **Ab in die Lobby.**
6. Links siehst du jetzt alle Channels und wer gerade wo ist. **Ein Klick auf einen
   Channel**, und du bist drin. Wer spricht, bekommt einen farbigen Rahmen. Alles aus
   Schritt 4 und 5 findest du später unter **Ich → Audio und Sprechen**; mit `Esc` geht
   es zurück.

Der Link funktioniert genau einmal. Danach kennt yAPPA dich auf diesem Rechner – beim
nächsten Start bist du sofort drin.

**Bitte Headset benutzen.** Mit Lautsprechern hören die anderen sich selbst als Echo – eine
Echounterdrückung gibt es noch nicht.

**Kein Bluetooth-Headset im Freisprechmodus.** yAPPA braucht Geräte mit 48 000 Hz;
Bluetooth-Headsets schalten beim Mikrofon auf 16 kHz und werden abgelehnt. USB oder Klinke
geht. Lehnt yAPPA dein Gerät ab („has no 48 kHz mode“), in Windows unter *Sound →
Eigenschaften des Geräts → Erweitert* das Format auf **48000 Hz** stellen.

## Was die Knöpfe tun

Unten links sitzt immer der **Sprach-Kasten** – egal, was gerade offen ist, der Call läuft
weiter.

| Knopf | Wirkung |
| --- | --- |
| Mikrofon (im Sprach-Kasten) | Dein Mikrofon stummschalten. Der Balken zeigt weiter an, dass es funktioniert. |
| Kopfhörer (im Sprach-Kasten) | Alle anderen stumm für dich. |
| Verlassen | Raum verlassen. |
| Regler bei einer Person | Diese Person für dich lauter oder leiser machen. |
| Hörbar / Stumm für mich | Diese eine Person nur für dich stummschalten. Ein Klick auf den Namen links zeigt dasselbe noch einmal größer. |
| Ich → Profi → Diagnose | Die Messwerte (siehe unten). Ein ⓘ neben einem Wert erklärt ihn. „Profi“ ist der Umschalter oben in „Ich“; er zeigt auch Signalkette, Tasten, Netzwerk und Updates. |
| Ich → Hilfe | Diese Anleitung, „So funktioniert yAPPA“ und ein Glossar – direkt in der App, auch ohne Verbindung. |
| Suchen oder springen (`Strg K`) | Springt überallhin: Einstellungen, Leute, Stummschalten, Verlassen. |
| `Esc` | Einen Schritt zurück. |

## Wenn etwas komisch klingt

Genau dafür testen wir. Bitte so melden, dann können wir es nachvollziehen:

1. **Ich → Profi → Diagnose** öffnen und kurz schauen, ob etwas orange ist – z. B. „Paketverlust" oder
   „Ersetzte Samples". Das gern dazuschreiben.
2. Dort auf **Diagnose-Datei speichern** klicken. Die Datei landet im Ordner
   „Downloads" (der genaue Pfad wird angezeigt).
3. Die Datei an Chris schicken, zusammen mit: **Was** ist passiert (Knacken, Aussetzer,
   Roboterstimme, jemand war weg) und **ungefähr wann** (Uhrzeit reicht).

In der Datei stehen Programmversion, deine Einstellungen, die Gerätenamen und ein
Protokoll mit Messwerten pro Sekunde. Gespräche werden nicht aufgezeichnet, und deine
Einladung steht nicht drin.

## Bekannte Lücken

- Die **Rauschunterdrückung** ist an, und das Mikrofon geht nur auf, wenn du sprichst
  (Sprachaktivierung). Kommt deine Stimme abgehackt oder zu spät an – vor allem, wenn du
  leise sprichst –, sag Bescheid: die Schwelle lässt sich noch nicht in der App einstellen.
- Noch kein Push-to-Talk-Knopf.
- Geräte müssen auf 48000 Hz stehen (Windows-Soundeinstellungen → Gerät → Format). Meldet
  yAPPA „kein 48-kHz-Modus", dort umstellen.
- Kein Text-Chat, kein Soundboard.
- Updates kommen noch von Hand: neuer Installer, drüberinstallieren.
- Abmelden (**Ich → Konto**) heißt: Für den nächsten Start brauchst du einen neuen Link.
- Stürzt yAPPA bei jemandem ab, steht er für die anderen noch etwa 20 Sekunden im Channel.
