# So funktioniert yAPPA

Für alle, die wissen wollen, was hinter den Knöpfen passiert. Keine Vorkenntnisse nötig;
die fetten Begriffe stehen im [Glossar](glossar.md) genauer erklärt.

## Drei Teile

yAPPA besteht aus drei Teilen, die bewusst getrennt sind:

| Teil | Was er tut | Wo er läuft |
| --- | --- | --- |
| **App** | Mikrofon aufnehmen, Stimmen abspielen, Knöpfe | auf deinem Rechner |
| **Server** | Stimmen zwischen allen verteilen | bei einem von uns zu Hause oder auf einem Mietserver |
| **Verwaltung** | Einladungen, Channels, Mindestversion | neben dem Server |

Der Server mischt nichts zusammen. Jede Stimme kommt einzeln bei dir an, deshalb kannst du
jede Person für dich lauter, leiser oder stumm stellen. Die Verwaltung ist nie im Sprachweg:
fällt sie aus, läuft das Gespräch weiter.

## Der Weg deiner Stimme

1. **Mikrofon.** Die Soundkarte liefert den Ton in kleinen Portionen (**Puffer**), mit
   48 000 Messungen pro Sekunde (**Abtastrate**). Der Balken unter dem Mikrofon zeigt den
   **Pegel**.
2. **Verpacken.** Alle 10 Millisekunden wird ein Stück Ton mit dem **Codec** Opus
   verkleinert. Jedes Paket enthält zusätzlich eine Kopie des vorherigen (**RED**), damit
   ein verlorenes Paket nicht auffällt.
3. **Netz.** Die Pakete gehen per UDP direkt zum Server. UDP wartet nicht auf Bestätigungen
   – ein zu spätes Paket ist für Sprache wertlos, also wird es lieber übersprungen. Wie
   lange der Weg dauert, zeigt die **Laufzeit zum Server**; wie unregelmäßig die Pakete
   ankommen, der **Jitter**.
4. **Server.** Er reicht dein Paket an alle anderen im Raum weiter, ohne es zu öffnen.
5. **Empfang.** Bei den anderen sammelt ein **Jitterbuffer** die Pakete und gleicht
   Schwankungen aus. Fehlt eines, errät der Decoder den Inhalt (**ersetzte Samples**).
6. **Ausgabe.** Die Stimmen aller werden gemischt – mit deinem Regler pro Person – und an
   den Kopfhörer gegeben.

Alles zusammen soll 70 bis 120 Millisekunden dauern (**Latenz**). Gemessen ist bisher der
digitale Teil mit 50 bis 60 ms; der Rest sind Mikrofon- und Ausgabepuffer.

## Warum das so gebaut ist

- **Sprachqualität und Stabilität zuerst.** Alles, was mit Ton zu tun hat, läuft in
  nativem Code außerhalb der Oberfläche, damit ein hängendes Fenster nie den Ton stört.
- **Selbst gehostet.** Der Server gehört uns, es gibt kein Konto bei einer Firma und keine
  Werbung. Deshalb braucht es jemanden, der ihn betreibt – siehe „Homeserver einrichten“ im
  Projekt-Bereich.
- **Feste Gruppe.** yAPPA ist für bis zu 20 Leute, die sich kennen. Darum reicht eine
  **Einladung** als Zugang, und darum gibt es keine Freundeslisten oder öffentliche Server.
- **Windows zuerst.** Die meisten von uns spielen unter Windows; macOS baut aus demselben
  Code und bekommt, was mitkommt.

## Was noch fehlt

Im Moment hat yAPPA Channels, eine **Rauschunterdrückung** (DeepFilterNet) und
Sprachaktivierung, aber noch kein **Push-to-Talk** und keine **Echo**-Unterdrückung. Die
Reihenfolge, in der das kommt, steht unter „Status“ im Projekt-Bereich. Was dort unter
„Später“ steht, ist bewusst noch nicht gebaut.
