# Claude Code end to end, tenth run

One card, written from the first hour of real use of 0.3.1 and run by a Claude Code agent (Sonnet)
inside the owner's installed Toknila.app 0.3.1, on the owner's own TOKNILA project in **local** merge
mode, 13 November 2026. The owner said that after an answer in the chat the microphone button no
longer pressed; the orchestrator read the Mac's unified log, wrote the cause into the card, and the
chain (`phase-8`, autostart) ran it to a release.

Result: the card reached **Merged**; the chain released **0.3.2**.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-116 | A refused microphone says why: the recognizer's reason reaches the person | 2 | returned once by the person: a guard without a test, a wait on time, red-first evidence missing |

## What the log said

- The window asked macOS for speech recognition and the microphone once, at the first press; both
  were granted (`AUTHREQ_PROMPTING` then created, in the TCC log).
- Every later press: the WebKit GPU process started the audio capture and stopped it within four
  milliseconds, and the on-device recognizer `localspeechrecognition` refused with
  `kLSRErrorDomain Code=201 "Siri and Dictation are disabled"`. The owner has Siri and Dictation
  off.
- The first session worked because the on-device speech model was not yet installed: WebKit forces
  on-device recognition only when the device supports it, so the first request went to the server
  recognizer while the model downloaded; every later request was forced on-device and refused.
- WebKit maps every failure of its recognition task to the Web Speech error `aborted` and puts the
  system's sentence into the event's `message`. The hook read only the code, and the store mapped
  `aborted` to idle with no sentence, so a failure the system had explained became silence.

## Findings

1. **An explained failure was mapped to silence.** The fix is the card; the rule is in AGENTS.md
   under Verification discipline: a failure the system explains reaches the person as a sentence
   with the system's reason, and no error code maps to silence except the one the person caused.
   It holds for every surface, not only the microphone.
2. **An agent cannot see the window it changes** (again, as in run 9). The card named the
   real-window steps as the person's (PERSON), and the hand-in listed them that way instead of
   claiming them. This is the shape a real-window card should keep.
3. **The reviewer approved a guard nobody had seen fail.** Round 2 noted that the ended-at-once
   guard had no test and that no test had been seen red, and approved anyway. The person returned
   the card for a test that drives the guard through the fake recognizer, a wait on a condition
   rather than on time, and red-first output quoted in the hand-in; the second hand-in carried all
   three. The reviewer's instructions (the verdict section of the protocol) now say it: a gap the
   reviewer can name, a test the task asks for and the hand-in lacks, a guard no test drives, a
   test nobody has seen fail, is a point of `request_changes`, never a remark under `approve`.
4. **The orchestrator's own tools.** `log` in zsh is a builtin, so the Mac's unified log is read
   with `/usr/bin/log show`. A wait of 200 ms kept in the e2e after the condition wait is the one
   smell left in the card; it is noted in the Accept remarks.

## What the owner can do now that was not possible before

Press the microphone with Siri and Dictation off and read why macOS refused and where to turn
Dictation on, instead of a button that falls back as if nothing was pressed; any other refusal of
the recognizer shows its reason; a recognizer that ends at once without words says so.
