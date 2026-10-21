# TeslaCam Viewer Design System

Shared by `apps/web` (React) and `apps/mac` (GPUI). Token values live in
`packages/contract/design-tokens.json`; both apps read them from there and their tests fail when a
platform copy drifts. UI copy is Japanese and identical on both platforms.

Reference prototype: [`docs/design/2026-10-20-garage-floor.html`](docs/design/2026-10-20-garage-floor.html)
(open it through any static server; the camera images in it are stand-ins).

## 0. Research Log

- 2026-08-21 – 2026-08-25: the first system ("light workspace / evidence theater") separated a light work
  surface from a black video theater, removed AI-template tells and fixed density issues. Its behavioural
  rules (privacy, copy rules, inspector fields, autoplay, day navigation, dense history) still hold and are
  carried over below.
- 2026-10-20 redesign audit: the owner found the 2026-08 UI hard to read and "business-like". Causes: boxed
  panels and dividers everywhere, a heavy black theater, a 240 px inspector listing raw fields, a row of
  equal-weight buttons under the video. Tesla is fun to use; the viewer should feel like the car.
- Reference: ROJI (`TSUQREA/mapbox-ai-demo`) — the map is the canvas, panels float above it with a 22 px
  radius and one soft shadow, neutral surfaces, one colour per meaning, Barlow numerals readable at arm's
  length, pill controls. Tesla: `#171A20` ink and `#3E6AE1` blue from tesla.com, the top-down car render as
  the home of every camera, very little chrome.
- Decision: **Garage floor**. The car is the map. The six camera windows sit around the car where the
  cameras are mounted, on a light floor; the library, the event summary and the transport float above it.

## 1. Identity

You plug the car's USB drive into a Mac to find a moment — something approached the car during Sentry, a
clip you saved with the horn, a drive from this morning — and watch all six cameras at once. The screen is
a calm, light garage floor with the car in the middle. Video is the only dark thing on it. Numbers (times,
counts, durations) are set large in Barlow, whose shapes come from California highway signs.

Principles:

1. **The car is the map.** Every camera window is placed by its mount position on the car. A soft
   field-of-view wedge connects the mount point to its window; the active camera's wedge is blue. This is
   the one bold element. Nothing else competes with it.
2. **Float the controls, keep the floor still.** Only floating layers (library, top pills, transport,
   dialogs, popovers) have a shadow. Camera windows have none.
3. **Numbers you can read from across the room.** Clock, event times, counts: Barlow 600, tabular.
4. **One colour per meaning.** Blue = what you are doing now (selection, playhead, primary action).
   Red = Sentry only. Green = saved clips only. Grey = recent drives. Never colour alone: every category
   also has its icon and word.
5. **Say what it is.** Short plain Japanese, no meta strings joined by `·`, no English all-caps labels,
   no raw field dumps on the main screen.

## 2. Colour

| Token | Value | Usage |
|---|---|---|
| `floor` | `#E9EBEE` | App background (with `floor-light` radial highlight behind the car) |
| `floor-light` | `#F7F8FA` | Centre of the floor highlight |
| `surface` | `#FFFFFF` | Solid surfaces, selected segmented item, scrubber thumb |
| `glass` | `rgb(255 255 255 / .94)` + `blur(24px) saturate(1.4)` | Floating layers |
| `ink` | `#171A20` | Text, play button, hover rings |
| `ink-2` | `#5C5E62` | Secondary text |
| `ink-3` | `#6B6E74` | Tertiary text, axis labels (5.1:1 on white, 4.6:1 on `fill`) |
| `line` | `#E2E4E8` | The few separators that remain |
| `fill` | `#F2F3F5` | Segmented tracks, chip and icon-button backgrounds, scrubber track |
| `fill-hover` | `#E9EBEE` | Hover on `fill` controls |
| `drive` | `#3E6AE1` | Primary action, selection ring, playhead, active wedge, focus ring |
| `drive-hover` | `#3459C4` | Primary action hover |
| `drive-soft` | `rgb(62 106 225 / .14)` | Selected event row |
| `sentry` | `#E31937` | Sentry markers, detection marker |
| `sentry-ink` | `#C8102E` | Sentry text on `sentry-soft` (AA for 11 px semibold) |
| `sentry-soft` | `rgb(227 25 55 / .10)` | Sentry chip background |
| `saved` | `#12805A` | Saved markers |
| `saved-ink` | `#0E6B4B` | Saved text on `saved-soft` |
| `saved-soft` | `rgb(18 128 90 / .10)` | Saved chip background |
| `recent` | `#5C5E62` | Recent chip text (on `fill`) |
| `recent-marker` | `#9AA0A8` | Recent markers on the day rail |
| `warning` | `#8A5A00` | Missing camera / scan warnings text |
| `warning-soft` | `#FFF4DC` | Warning background |
| `video` | `#000000` | Video background and letterbox inside camera windows |
| `video-missing-a` / `video-missing-b` | `#15181D` / `#181B21` | 135° stripes of a camera window without footage |
| `video-label-scrim` | `rgb(0 0 0 / .55)` | Camera name pill over video |
| `video-text` | `#FFFFFF` | Text over video |
| `video-muted` | `#C2C8D0` | Secondary text over video |
| `scrim` | `rgb(23 26 32 / .32)` | Behind modal dialogs |
| `shadow-float` | `0 1px 2px rgb(23 26 32 / .06), 0 14px 40px rgb(23 26 32 / .12)` | Floating layers only |
| `shadow-control` | `0 1px 3px rgb(23 26 32 / .12)` | Selected segmented item |
| `wedge` | `ink` at 7 % → 0 % | Field-of-view wedge, inactive |
| `wedge-active` | `drive` at 28 % → 0 % | Field-of-view wedge, active camera |

Rules: never introduce a colour outside this table; add it here and to the JSON first. Focus is a 2 px
`drive` outline with 3 px offset everywhere.

## 3. Typography

| Role | Family | Weight / size / line-height | Usage |
|---|---|---|---|
| `display` | UI | 600 / 30 px / 1.3 | Welcome headline |
| `title` | UI + Num | 600 / 20 px / 1.3 (digits 22 px Num) | Day title in the library |
| `clock` | Num | 600 / 34 px / 1 | Current playback time |
| `numeral-l` | Num | 600 / 24 px / 1.05 | Event start time in the list |
| `numeral-m` | Num | 600 / 18 px / 1.2 | Time in the event pill, counts in the segmented control (15 px) |
| `body` | UI | 400 / 14 px / 1.9 | Welcome paragraph, dialog text |
| `ui` | UI | 500–600 / 13 px / 1.4 | Buttons, pill text, details rows |
| `small` | UI | 400 / 12 px / 1.5 | Facts under an event, camera labels |
| `micro` | UI / Num | 600 / 11 px / 1.4 | Chips, rail hours, footnotes |

- UI family: `"Hiragino Sans", "Hiragino Kaku Gothic ProN", "Noto Sans JP", system-ui, sans-serif`
  (Mac app: the system font, which resolves to Hiragino Sans for Japanese).
- Num family: **Barlow** 500/600, bundled with both apps from `packages/assets/fonts/` (OFL), tabular figures. Use it for digits and for the Latin wordmark only.
- No all-caps, no letter-spaced eyebrows, no monospace.

## 4. Shape, depth and spacing

- Radii carry hierarchy: floating panel and transport `22`, dialogs `22`, camera windows `14`, event rows
  and segmented tracks `14–16`, thumbnails `10`, pills / chips / round buttons `999`.
- Glass + `shadow-float` only on floating layers. The floor gets a single radial highlight centred on the
  car. Camera windows: no border, no shadow; hover = 2 px `ink` ring with a 2 px floor gap; active = the
  same ring in `drive`.
- Spacing unit 4 px. Gutter around floating layers: 16 px. Minimum hit target: 36 px inside floating
  layers on desktop, 44 px on touch layouts (web ≤ 1100 px).

## 5. Layout

### Desktop (web ≥ 1101 px, Mac window ≥ 1100 × 700)

```
┌───────────────┬──────────────────────────────────────────────────────────────┐
│ Library 372px │ (event pill: category · start · place · detection)  (view | 書き出す) │
│ wordmark  📁  │              ┌──────── front ────────┐                        │
│ segmented     │   ┌─ L pillar ─┐  \     |     /   ┌─ R pillar ─┐             │
│ ‹ 8月20日 ›   │   │            │   ╲   car   ╱    │            │             │
│ 0 ··●··●· 24  │   └────────────┘    ╲  ▮▮  ╱     └────────────┘             │
│ event rows    │   ┌─ L repeater┐   ┌── back ──┐   ┌─ R repeater┐             │
│ …             │   └────────────┘   └──────────┘   └────────────┘             │
│ warnings      │        ( ‹  ▶  0:15/1:00  ━━━━●━━━━▲━━━━  ⟲10 ⟳10  1×  › )      │
└───────────────┴──────────────────────────────────────────────────────────────┘
```

- Library: floating, top/left/bottom 16 px, width 372 px (320 px below 1280 px).
- Top row: event pill left-aligned to the stage, actions pill right-aligned; both 46 px high.
- Stage: CSS grid / GPUI flex with areas `"lp front rp" "lp car rp" "lr back rr"`, rows `.8fr 1.12fr
  .86fr`, columns `1fr 1.18fr 1fr`, gaps 14 × 26 px. Windows are 4:3, `contain`-fit; the back window is
  86 % of its column. The car (`vehicle-top.png`, 1:2) fills the middle row height with a soft elliptical
  floor shadow.
- Transport: floating, bottom 16 px, centred on the stage, max width 920 px.
- Text alignment: left everywhere except numerals inside controls.

### Tablet (web 741–1100 px)

Library collapses to 320 px and can be hidden with a toggle in the event pill; icon-only view switch and
export; skip buttons move into a `…` menu; touch targets 44 px.

### Phone (web ≤ 740 px)

The library is the first screen (full width, same content). Choosing an event pushes the viewer with a
`一覧へ` back button. The viewer shows one camera large; a **mini car** below it (the car glyph with six
tappable mount dots, each with a visible label underneath) switches the camera. The transport is fixed to
the bottom (≤ 140 px). All six `<video>` elements stay mounted for sync; only the selected one is visible.

## 6. Components

### Library (floating left)

- **Header**: `TeslaCam Viewer` wordmark (Num 600 15 px), search icon button (opens an inline field with
  placeholder `日時またはカテゴリ`), folder chip with the folder name (`別のフォルダを選ぶ` as accessible
  name).
- **Category segmented control**: `すべて` / `最近の走行` (shown as `走行`) / `保存済み` (shown as `保存`) /
  `セントリー`, each with its count in Num 600 15 px. The accessible name keeps the full category name.
- **Day**: `‹ 8月20日 木曜日 ›`. Chevrons move to the previous / next day that has recordings (skipping
  empty days), accessible names `前の記録日` / `次の記録日`. The title opens a date picker. Changing the day
  selects and plays the newest event of that day.
- **Day rail**: 0–24 h track (`fill`, 4 px) with one 8 × 16 px marker per event coloured by category; the
  current event marker is 10 × 24 px with a white ring. Markers whose 44 px hit areas overlap become a
  cluster pill showing `20:35–20:45` and the count; activating it lists those events in the rows below.
  Left/Right moves between markers, Home/End to the first/last of the day.
- **Event rows**: 92 px 4:3 thumbnail (only when `thumb.png` exists; otherwise a quiet dark tile with the
  category icon), start time `numeral-l`, category chip, facts (`2分`, `6カメラ`, Sentry detection time
  `20:35:30に検知`). Missing cameras: one warning line naming them, e.g. `右リピーターの記録がありません`.
  Selected row: `drive-soft` background. Rows are buttons; the list is virtualised.
- **Footer**: scan warnings as one sentence each (e.g. encrypted clips skipped), `micro` `ink-3`.

### Event pill and details (floating top)

- Pill: category chip (30 px), start time `numeral-m` + `開始`, place (city, street from `event.json`,
  pin icon), Sentry detection (`物体の接近を20:35:30に検知` style: reason in words + time in `sentry-ink`).
  Items drop from the right as the window narrows: detection, then place.
- Activating the pill opens a details popover (glass, 22 px radius, 360 px) with rows, shown only when the
  data exists: `場所（Teslaの推定値）`, `推定座標`, `検知`, `再生開始`, `カメラ`, `長さ`, `セグメント`,
  `録画開始`, and a closed `技術情報` disclosure with the raw `reason`. No external map links, no reverse
  geocoding: location never leaves the device.

### Actions pill (floating top right)

- View segmented: `車両配置` (car icon) / `6分割` (grid icon) / `代表映像` (only when `event.mp4` exists).
- `書き出す` (download icon). During export it becomes a progress button (`書き出し中 42%` with a ring);
  activating it offers `中止`.

### Stage

- **Camera window**: 4:3, radius 14, `video` background, `contain`-fit. Name pill bottom-left
  (`フロント`, `左ピラー`, `右ピラー`, `左リピーター`, `リア`, `右リピーター`). States: loading (spinner +
  `読み込み中`), ready, missing (stripes + camera-off icon + `記録がありません`), unavailable
  (`映像を読み込めません`), corrupt (`映像が破損しています`), unsupported (`未対応の映像形式です`).
- Click / Enter expands that camera to fill the stage; the other five shrink into a strip of mini windows
  along the stage's bottom edge, and a mini car with mount dots sits at the strip's left for spatial
  switching. `Esc` or the same window returns to `車両配置`.
- **Wedges**: drawn under the windows from the mount point to the two window corners that face the car.
  Mount points (relative to the car image box): front 50 %/22 %, left pillar 16 %/46 %, right pillar
  84 %/46 %, left repeater 10 %/31 %, right repeater 90 %/31 %, back 50 %/93 %. The active camera shows a
  `drive` mount dot with a white ring.
- **6分割**: 3 × 2 grid of the same windows (front, back, left pillar, right pillar, left repeater, right
  repeater — reading order), no car, no wedges.
- **代表映像**: one large window with native-like controls; the six-camera clock pauses.

### Transport (floating bottom)

`‹` previous event (`過去の記録`), round 58 px `ink` play / pause button, clock (`0:15` in `clock`,
`/ 1:00` in Num 500 13 px `ink-3`), scrubber, `⟲10` / `⟳10` (`10秒戻る` / `10秒進む`), rate pill
(`1×`; a menu with 0.5× / 1× / 1.5× / 2×, accessible name `再生速度`), `›` next event (`新しい記録`).
Scrubber: 6 px `fill` track, `drive` fill, 20 px white thumb with a 2 px `drive` ring; segment boundaries
as 2 × 12 px ticks; Sentry detection as a `sentry` tick labelled `検知`; when playback started at the
pre-roll, the start shows a hollow tick labelled `検知の15秒前`.

### Welcome, scanning, errors

- Welcome lives in the library panel while the stage shows the car with six empty windows (white 55 %,
  camera names centred). Headline `TeslaCamの記録を見る`, paragraph, primary pill `フォルダを選ぶ`
  (50 px, `drive`), two short notes at the panel bottom (Sentry pre-roll, composite export). Web adds the
  browser note `MacのChromeまたはEdgeに対応しています。` under the button.
- Scanning: the button becomes a progress block — `読み込み中`, the processed count in `numeral-l`
  (`1,250件を確認しました`), an indeterminate 4 px bar. Cancelable.
- Errors replace the paragraph with what happened and what to do, plus the retry button; never apologise.

### Composite export dialog

Glass dialog, 22 px radius, max 520 px, centred on the stage over `scrim`. Shows the event, the full
range, the layout (a thumbnail of the export frame), `1280 × 720`, the detected format, and
`書き出しを開始`. After completion the actions pill shows `MP4を保存` / `WEBMを保存` on the web. The Mac
asks for the destination with the native save panel when `書き出しを開始` is pressed, so its finished state
shows `書き出し完了` with `Finderで表示` instead.

### Export frame (1280 × 720, both platforms)

The saved video looks like the app. Rects are in `packages/contract/composite-layout.json`:

| Slot | Rect (x, y, w, h) |
|---|---|
| front | 464, 12, 352, 264 |
| left_pillar | 56, 112, 352, 264 |
| right_pillar | 872, 112, 352, 264 |
| left_repeater | 56, 444, 352, 264 |
| right_repeater | 872, 444, 352, 264 |
| back | 480, 456, 320, 240 |
| vehicle | 598, 285, 84, 168 |

- Background `floor`; every video `contain`-fit inside its slot; the fitted video rect gets 12 px rounded
  corners (implemented as one static overlay image: `floor` everywhere except rounded holes, plus the
  neutral wedges, the car and the labels). The overlay is rendered once per export.
- Labels: pill at the fitted rect's bottom-left, inset 10, height 24, padding 10, UI 500 13 px,
  `video-label-scrim`, `video-text`. Missing camera: `video-missing-a` fill, centred `記録なし` in
  `video-muted`, label pill as usual.
- 30 fps H.264 MP4 (WebM fallback on web only), segments concatenated in order, progress 0–1 over the
  whole event.

## 7. Motion

| Token | Duration | Easing | Usage |
|---|---:|---|---|
| `motion-fast` | 160 ms | `cubic-bezier(.2, 0, 0, 1)` | Hover, pressed, focus, selection colour |
| `motion-sheet` | 320 ms | `cubic-bezier(.22, .8, .24, 1)` | Expanding a camera, opening the details popover, phone push |

- One orchestrated moment: when a folder finishes loading, the six empty windows fill with video in mount
  order (front first), 40 ms apart. Nothing else animates on its own.
- Motion answers actions only. `prefers-reduced-motion` (web) / "Reduce motion" (macOS) sets all durations
  to 0.

## 8. Copy

- Japanese, plain verbs, sentence-final punctuation only in paragraphs.
- Actions keep one name through the flow: `書き出す` → `書き出し中` → `書き出し完了` → `MP4を保存` (web) /
  `Finderで表示` (Mac).
- Healthy states are stated positively (`6カメラ`); faults name the direction (`右リピーターの記録がありません`).
- No `·`-joined meta strings, no `0台のカメラがありません`, no English labels except the wordmark.

## 9. Accessibility

- WCAG 2.2 AA: 4.5:1 text, 3:1 large text and control boundaries (every pair in §2 is checked by a test
  over `design-tokens.json`).
- Everything keyboard reachable; visible focus ring; segmented controls are radio groups; the scrubber is
  a slider with `aria-valuetext` like `0分15秒 / 1分`.
- Category, warnings and states always pair colour with icon + word.
- Keyboard (ignored while focus is in a text field or a button): `Space` play/pause, `←`/`→` −10 s / +10 s,
  `1`–`6` expand that camera (front, back, left pillar, left repeater, right pillar, right repeater; the same
  key again returns), `Esc` back to the vehicle layout, `[`/`]` previous / next event (new).
- Reduced motion and forced colours (web: selected items get a 3 px `Highlight` outline) are respected.

## 10. Platform mapping

| Concept | Web (`apps/web`) | Mac (`apps/mac`) |
|---|---|---|
| Tokens | CSS custom properties `--<token>` in `styles/tokens.css`, verified against `design-tokens.json` | `Theme` struct built from `design-tokens.json` (`include_str!`), verified by a unit test |
| Floating glass | `background: glass; backdrop-filter` | `glass` colour; no blur (GPUI) |
| Barlow | `@font-face` importing `packages/assets/fonts/` (bundled by Vite) | Registered at startup from `packages/assets/fonts/` (`include_bytes!`) |
| Car image | `/vehicle-top.png` | `apps/web/public/vehicle-top.png` via `include_bytes!` (same file) |
| Wedges | SVG polygons with linear gradients, recomputed on resize | `canvas()` paint paths with gradient fills |
| Video | `<video>` per camera | `surface()` per camera from AVPlayerItemVideoOutput |
