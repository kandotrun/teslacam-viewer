# Task 8 ブラウザ操作・レスポンシブ検証レポート

## 完了内容

- Chromium Playwrightをproduction build/previewに接続し、web標準形状のDirectory Handleと実際のdirectory input fallbackを検証した。
- 非機密の合成H.264/yuv420p/faststart MP4をfixtureとして追加した。320×180、2.0秒、2,394 bytes、SHA-256は`b6550d190edf22c0634026d8424b76119bc53e3d0ace82483015877bd4968250`。
- 1440×1000、1024×768、390×844のresponsive状態を実ブラウザで検証した。
- mobileは6個のvideo DOMを維持したまま1方向だけを表示し、ラベル付き6方向switcherを局所横スクロールにした。
- tablet/mobileのイベント一覧をアクセシブルに開閉可能にし、イベント選択後はシアターを固定transportの上へ配置した。
- native picker非対応時の互換statusと、ユーザー操作中に直接file inputを開くfallbackを追加した。
- terminal grid、代表映像、transport、およびterminal tile内ラベル/statusの重なりを解消した。
- welcome見出しの「ひとつのシアターで。」を意味単位で保持し、全viewportで1行であることを実座標で検証した。
- EventThumbnailの任意`getFile()` rejectionとunmount後解決を安全な空thumbnailへ収束させた。Appのprimitive rejectionもscan failureへ収束させた。

## TDD記録

各修正前の失敗結果は`.omo/evidence/teslacam-viewer/red/`、修正後は`green/`または`gates/`に保存した。

- thumbnail unknown rejection: `red/event-thumbnail-unknown-rejection.txt`
- App primitive rejection: `red/app-primitive-rejection.txt`
- responsive/fallback controls: `red/responsive-controls-and-fallback.txt`
- 1024px disclosure: `red/tablet-1024-disclosure.txt`
- terminal major-region geometry: `red/terminal-overlap-geometry.txt`
- terminal internal pill geometry: `red/terminal-internal-overlap.txt`
- welcome phrase/mobile dock geometry: `red/mobile-unoccluded-cjk-wrap.txt`、`red/mobile-unoccluded-after-collapse.txt`
- final responsive geometry: `gates/responsive-unoccluded-cjk-wrap-final.txt`

## Browser scenarios

Invocation: `npx playwright test`

Result: 6 tests passed、0 failed、0 skipped、exit 0。最終実行は`gates/playwright-round4-json-final.txt`、machine-readable resultは`json/playwright-results.json`。

1. welcomeとloadedを3指定viewportでcapture。250+ file progress、focus、semantic heading wrap、tablet disclosure/theater、mobile cameraとdock非重複、page overflowなしを検証。
2. 全category switch、event selection、6 ready videos、play/pause、elapsed advance、Space/矢印/数字/Escape、前後event、44px controls、HTTP(S) requestのfilename/media-byte leakなしを検証。
3. missing camera、corrupt/unsupported terminal、representative fallback、grid/代表映像/transport/terminal pillsの非重複を検証。
4. wrong folder、permission denied、cancel、primitive picker rejection、console/page errorなしを検証。
5. `webkitRelativePath`を持つ実directory-input fallbackをfile chooser経由で検証。
6. mobileの6 video DOM/1 visible camera/local switcher overflow/全6到達、reduced motion、expanded状態を検証。

## Screenshot evidence

- desktop 1440×1000: `desktop-welcome.png`、`desktop-loaded.png`、`desktop-expanded.png`、`desktop-missing.png`、`desktop-terminal-fallback.png`
- tablet 1024×768: `tablet-welcome.png`、`tablet-loaded.png`
- mobile 390×844: `mobile-welcome.png`、`mobile-loaded.png`、`mobile-switcher.png`

Path: `.omo/evidence/teslacam-viewer/screenshots/`

全captureは合成timestampと黒い合成映像のみを含む。実車映像、住所、座標、VIN、ナンバー、実SSD metadataは含まない。

## Gates

| Gate | Result | Artifact |
|---|---|---|
| `npm run typecheck` | PASS | `gates/typecheck-round4.txt` |
| `npm run lint` | PASS | `gates/lint-round4.txt` |
| `npm test -- --run` | 13 files / 106 tests PASS | `gates/unit-tests-round4.txt` |
| `npm run build` | PASS | `gates/build-round4.txt` |
| `npx playwright test` | 6/6 PASS | `gates/playwright-round4-json-final.txt` |
| `npx wrangler deploy --dry-run` | PASS | `gates/wrangler-round4.txt` |
| `npm run react-doctor -- --verbose` | exit 0、81/100 | `gates/react-doctor-round4.txt` |
| `git diff --check` | PASS | evidence manifest記載 |

React Doctorの4 warningは、既存unused dev dependency、明示的release/disposeを持つObject URL registryへのfalse positive、進捗を保つ逐次directory走査、既存Zod 3 APIである。Task 8の変更による未処理runtime errorはない。

## Visual review

最終同一ソース/build/JSON/10 screenshotsを、clone/design-system/integrity系とvisual/CJK precision系の2 reviewerが独立に確認した。

- `visual_integrity_pass4`: APPROVE / PASS、blockerなし。
- `visual_cjk_pass4`: PASS / APPROVE、高信頼度、blockerなし。

判定全文は`.omo/evidence/teslacam-viewer/visual-review-final.md`に記録した。

Imagen原本とのpixel cloneは本件の要求ではなく、DESIGNの参照は方向性に限定されるため、実装DOM、design tokens、指定viewportの製品品質を判定対象とした。

## Commit

Subject: `ブラウザ操作とレスポンシブ表示を検証`

このレポートを含むcommitのSHAは、commit後の`git rev-parse HEAD`で確定し、完了報告に記録する。

## Residual risks

- 自動E2EはChromiumのみ。実macOSのChrome/Edge directory pickerと実SSDはTask 9の手動受入対象。
- Playwright worker終了が過去に1回だけ300秒遅延したが、その後の複数fresh runと独立実行では再現せず、すべてexit 0で終了した。
- corrupt fixtureは意図的な4-byte合成dataであり、ブラウザcodec差はterminal statusを`corrupt`または`unsupported`の代表集合で検証する。
