# Web and desktop dictation

Completed and released on 2026-09-14. Public acceptance passed on `https://denchat.app`.

## Behavior

The composer has a microphone immediately before Send. Listening uses the accent, a 1.2-second pulse, and the mono typing-area label. Reduced motion removes the pulse. Partial recognition replaces the active segment at the caret; stop leaves editable text. Escape, tapping outside, hiding the page, and changing rooms release the microphone. Manual edits and sends cannot be overwritten by late recognition. Dictation never sends a message.

First use asks before downloading and requests microphone permission. Denial displays the brief's exact message and subsequent taps leave it visible until permission changes. Unsupported browsers hide the button. Dictation is hidden while joining or participating in a call, so it cannot change the call's microphone route.

Settings, Voice contains English, Punctuation, and model removal. The allowed `whisper-tiny.en` model supports only English; it cannot default to an unsupported device language. Punctuation off removes generated sentence punctuation while retaining apostrophes. These settings and the model are local to each device.

## Engine and cache

Pinned `@huggingface/transformers` 4.2.0 and `onnx-community/whisper-tiny.en` revision `2575352d61be1bf7225cf8f8b268a4678025fc58`. The q8 encoder and decoder total 40,843,851 bytes. The brief's 40 MB label describes the weights; tokenizer files and the bundled ONNX runtime also require storage and download bytes.

Audio stays in memory on this device. An AudioWorklet resamples to 16 kHz and a worker transcribes roughly every two seconds, with eight-second finalization windows and half-second overlap. Stop flushes the remaining audio. IndexedDB stores the pinned model and runtime assets. Remove clears only that dictation store. A previously loaded Den page can start dictation offline; this does not add offline navigation or a service worker to Den.

WebGPU is preferred when an adapter exists, with a fresh WASM worker on initialization failure. The v4 runtime requires its asyncify WASM module. Disabled ONNX graph optimization because its q8 MatMulNBits rewrite rejects this model. Direct construction of Whisper components avoids v4's uncached tokenizer discovery against `main`, which otherwise breaks offline use despite a pinned revision. Both choices were checked with real speech on GPU and WASM.

References: [Transformers.js WebGPU guide](https://huggingface.co/docs/transformers.js/guides/webgpu), [pinned model](https://huggingface.co/onnx-community/whisper-tiny.en/tree/2575352d61be1bf7225cf8f8b268a4678025fc58), and [upstream quantized graph failure](https://github.com/huggingface/transformers.js/issues/1707).

## Destructive actions

Settings, Machines now asks `Remove <name>? Its terminals end and it must be enrolled again.` inline, with danger Remove and quiet Keep buttons. Settings, Access uses the same component for Revoke and explains that access must be requested again. The first click cannot remove anything. Keep, focus leaving the confirmation, an outside tap, or eight seconds cancels it. Touch uses the same controls, with no native confirmation dialog.

The native stub smoke verifies both actions against real disposable server resources, including Keep, blur, the eight-second timeout, and touch confirmation. Screenshots show the mobile rows. Cleanup removed the temporary host, grant, request, and messages.

## Local verification

- `npm --prefix apps/web run check`: zero errors and warnings. Production build and dictation draft tests pass.
- Initial JS/CSS gzip increased from 181.78 KB to 184.90 KB, a 3.12 KB increase including both confirmations. Model, runtime, and transcription code are lazy assets.
- `scripts/dictation-smoke.mjs` uses Chromium's actual fake microphone and the public-domain JFK WAV referenced by Hugging Face's ASR documentation. The final local run entered speech in 4,075 ms after listening began.
- Passed consent/Not now, caret preservation, streaming replacement, reduced motion, desktop/mobile layout, Escape and microphone release, offline cached recognition, punctuation, model removal, permission denial, repeated denied tap, unsupported browser, outside stop, and manual-edit preservation. No audio upload or auto-send. Cleanup found zero new room artifacts.
- `scripts/m4-smoke.mjs` passed the native session and sidebar checks plus both confirmation flows on two disposable local servers. Screenshots were visually reviewed.

Native engine checks use the same built worker, actual spoken audio, and the desktop CSP inside WKWebView, WebView2, and WebKitGTK. These checks verify the engine in those webviews; they do not claim physical microphone acceptance in installed desktop apps. macOS and Windows both transcribed correctly with WebGPU and forced WASM. Linux WebKitGTK 2.50.6 exposed no WebGPU adapter and transcribed correctly with WASM.

Native eight-second speech fixture results, including model startup:

| Webview | Preferred backend | Total | Forced WASM total |
| --- | --- | ---: | ---: |
| macOS WKWebView, cached rerun | WebGPU | 3,934 ms | 3,914 ms |
| Windows WebView2 143 | WebGPU | 2,801 ms | 1,613 ms |
| Linux WebKitGTK 2.50.6 | WASM | 6,216 ms | 6,057 ms |

A cold macOS run took 92.7 seconds including loading, then a cached rerun spent 978 ms loading and 2,956 ms transcribing on GPU. WASM spent 1,139 ms loading and 2,775 ms transcribing. Download/startup time is separate from the browser smoke's ten-second limit after listening begins.

Private logs, fixtures, and native test programs are under `/mnt/storage/den-dictation`. Removed the temporary macOS executable/profile and Windows scheduled task/executable/profile after acceptance. Stopped the local test servers, Vite, and SSH tunnels. No production call was joined for these tests.

## Release

Published [v0.2.2](https://github.com/nhclink16/den/releases/tag/v0.2.2) from tag commit `d0f8269`. All six build jobs and publication passed in [Actions run 34872268238](https://github.com/nhclink16/den/actions/runs/34872268238). The release has 23 assets; its updater manifest contains signatures for Windows, Linux, and both macOS architectures. Downloaded Linux CI artifacts match the published SHA256SUMS.

Pulled main before deployment. `den-backup` completed successfully at 17:11:23 UTC with ExecMainStatus 0. Preserved the off-server archive as `/mnt/storage/den-dictation/before-v0.2.2.zip`, verified its ZIP entries, and recorded 16,903,840 bytes with SHA256 `16370807424ed248fcabd09d970db90052244042f01f74a75c905a6fe5aa2262`.

Ran `CARGO_TARGET_DIR=/mnt/storage/den-m9-target deploy/release.sh v0.2.2`. The live server SHA256 is `85386886fb58c19403612c8fd71fa81876c6f850a2bb7a374d73915ac207e47f`, matching the tagged artifact. All 14 public web files match their release hashes. The service is active, public health passes, and the parsed public OpenAPI document matches the pre-deployment baseline across all 70 paths, preserving M5b.

Desktop version is 0.2.2. Server/CLI version remains 0.2.1; this task changes no server code or shared schema.

The complete dictation smoke passed on denchat.app. Actual speech entered the composer in 3,669 ms after listening began, and cached recognition passed offline. Permission denial, unsupported-browser hiding, caret/partial text, punctuation, model removal, stop/edit behavior, microphone release, and both viewport widths passed without page errors. No audio upload or auto-send occurred. Cleanup found zero new messages, objects/requests, or uploads, and the temporary login session was logged out and removed. Public desktop/mobile screenshots were visually reviewed and remain in `/mnt/storage/den-dictation/public-shots`.

Committed screenshots: [desktop](shots/dictation-web-desktop.png), [mobile](shots/dictation-web-mobile.png), [machine removal](shots/dictation-web-machines-confirm.png), and [access revocation](shots/dictation-web-access-confirm.png).
