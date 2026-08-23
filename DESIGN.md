# TeslaCam Viewer Design System

## 0. Research Log

- Embedded refs: Tesla、Sentry、Runwayを候補化し、映像を主役にする静けさと精密な操作感を両立できるため、Layer Aはminimalist、Layer BはTeslaを採用した。ブランドロゴ、固有書体、公式UIは複製しない。
- Lazyweb: `video review workspace`、`security camera playback timeline`、`cinematic media library`の3クエリを実行し、Frame.io、Vimeo、Milestone XProtect、Soraの4画面を閲覧した。左のライブラリレール、時系列イベント列、主映像を支配的に置くシアター、画面下部に固定するトランスポートというレイアウト文法だけを抽出した。
- Imagen drafts: `exec-6d68dff4-c919-40dc-91e4-e86a587fda7c.png`、`exec-5cc54840-df75-4158-a0c4-9c38f47b1218.png`、`exec-053c7f42-d7cf-4d17-82c1-614fd8199a96.png`を比較し、1案目の`exec-6d68dff4-c919-40dc-91e4-e86a587fda7c.png`を参照契約として採用した。
- Target composition: チャコールのライブラリレール、イベント一覧、6カメラシアター、永続トランスポートバー。初期実装では同じトークンと状態規則によるwelcome shellだけを構築する。
- 2026-08-23 redesign audit: 公開版と375px、768px、1280pxの実画面を監査し、英語の大文字eyebrow、中央カード型welcome、均一な角丸枠、再生中の証拠文脈不足をAI生成サービスらしさの主因と特定した。Tesla、Apple、Raycastを候補に再比較し、映像そのものを主役にできるTeslaのradical subtractionを継続採用する。装飾は足さず、フォルダ、イベント時刻、カテゴリ、カメラ可用性を先に読ませる。

## 1. Atmosphere & Identity

SSDをMacへつなぎ、必要な記録を時刻から探し、映像を確認するためのローカル閲覧ツール。印象の核は、黒に近い面をわずかな明度差で積層する「チャコール・シアター」である。映像と証拠文脈が常に最初に読め、UIは静かに退く。高彩度色は主操作とカテゴリ状態に限定し、Tesla由来のradical subtractionをTeslaCam確認という独自用途へ再構成する。想定利用者は、SSDをMacへつないですぐ確認したい所有者、事故やセントリー記録を時刻から探す調査者、欠損カメラを含む大量イベントを長時間確認する運用者である。

AI生成サービスらしさを避けるため、宣伝見出し、英語の大文字eyebrow、意味のないカード、均一なpill、グラデーション、glow、glass、装飾アニメーションを使わない。日本語の短い作業語彙を使い、正常状態は肯定形、欠損や失敗は影響する方向を具体的に示す。

## 2. Color

### Palette

| Role | Token | Value | Usage |
|---|---|---:|---|
| Canvas | `--color-canvas` | `#090b0d` | 画面全体 |
| Rail | `--color-rail` | `#0e1114` | ライブラリレール |
| Surface | `--color-surface` | `#14181c` | イベント列、welcome panel |
| Surface raised | `--color-surface-raised` | `#1b2025` | カメラタイル、操作面 |
| Surface active | `--color-surface-active` | `#242a30` | hover、選択状態 |
| Text primary | `--color-text-primary` | `#f4f6f8` | 見出し、本文 |
| Text secondary | `--color-text-secondary` | `#aeb6bf` | 補助説明 |
| Text muted | `--color-text-muted` | `#7f8994` | メタデータ、非活性 |
| Border | `--color-border` | `#2a3138` | 面の境界 |
| Border strong | `--color-border-strong` | `#687581` | 強調境界 |
| Accent | `--color-accent` | `#3967d6` | 唯一の主操作、focus |
| Accent border | `--color-accent-border` | `#5b8def` | 選択、hover、拡大の操作境界 |
| Accent hover | `--color-accent-hover` | `#3c69d7` | 主操作hover |
| Accent active | `--color-accent-active` | `#315bc4` | 主操作active |
| Sentry | `--color-sentry` | `#eb5757` | セントリー。必ず文言・アイコンと併用 |
| Saved | `--color-saved` | `#5b8def` | 保存済み。必ず文言・アイコンと併用 |
| Recent | `--color-recent` | `#94a0ac` | 最近の走行。必ず文言・アイコンと併用 |
| Success | `--color-success` | `#62bd8f` | 完了、利用可能 |
| Warning | `--color-warning` | `#e1ad5a` | 欠損、注意 |
| Error | `--color-error` | `#f07178` | 破損、失敗 |
| Focus halo | `--color-focus-halo` | `#b9ccff` | 外側focus ring |

### Rules

- 無彩色面の明度差で階層を示し、グラデーションと装飾色は使わない。
- Accentは操作、Sentry・Saved・Recentは分類にだけ使う。
- 状態を色だけで伝えず、ラベル、Lucideアイコン、形状を併用する。
- CSSへ新しい色を直書きせず、先にこの表へ意味を追加する。

## 3. Typography

### Scale

| Level | Token | Size | Weight | Line height | Tracking | Usage |
|---|---|---:|---:|---:|---:|---|
| Display | `--font-display` | `clamp(2rem, 4vw, 3rem)` | 560 | 1.08 | `-0.03em` | 作業開始画面の主見出し |
| Display mobile | `--font-display-mobile` | `1.75rem` | 560 | 1.1 | `-0.03em` | 479px以下の作業開始見出し |
| Heading | `--font-heading` | `1.5rem` | 560 | 1.25 | `-0.02em` | パネル見出し |
| Body large | `--font-body-large` | `1.125rem` | 400 | 1.65 | `-0.01em` | リード文 |
| Body | `--font-body` | `1rem` | 400 | 1.6 | `0` | 標準本文、操作 |
| Small | `--font-small` | `0.875rem` | 450 | 1.5 | `0` | ブラウザ案内 |
| Label | `--font-label` | `0.75rem` | 560 | 1.35 | `0.01em` | カテゴリ、メタデータ |

### Font Stack

- Primary: `ui-sans-serif, -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Yu Gothic", sans-serif`
- Mono `--font-mono`: `ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace`
- Webフォント、外部フォント、ブランド固有フォントは読み込まない。

### Rules

- 日本語の単語と助詞が不自然に孤立しないよう、見出し幅と`line-break: strict`を併用する。
- 本文は14px未満にしない。見出しは4行以上になる前に流動サイズで縮小する。
- 1画面でweightを3種類までに抑え、太さだけに階層を依存しない。

## 4. Spacing & Layout

### Base Unit

基準単位は4pxとし、余白の意図は次のトークンで表す。

| Token | Value | Usage |
|---|---:|---|
| `--space-1` | `4px` | アイコンとラベル |
| `--space-2` | `8px` | 小さなまとまり |
| `--space-3` | `12px` | コンパクトな内側余白 |
| `--space-4` | `16px` | 標準間隔 |
| `--space-5` | `20px` | 操作群 |
| `--space-6` | `24px` | パネル内側余白 |
| `--space-8` | `32px` | 情報群の区切り |
| `--space-10` | `40px` | セクション間隔 |
| `--space-12` | `48px` | 大きな間隔 |
| `--space-16` | `64px` | ページ余白 |
| `--space-20` | `80px` | welcomeの呼吸 |
| `--library-rail-width` | `216px` | desktopライブラリレール幅 |
| `--event-list-width` | `336px` | desktopイベント一覧幅 |
| `--event-thumbnail-width` | `88px` | イベントカードのサムネイル幅 |
| `--event-card-min-height` | `96px` | イベント行の最小高 |
| `--theater-tile-min` | `120px` | desktop/tabletカメラタイル最小高 |
| `--theater-terminal-tile-min` | `80px` | 代表映像と併記するterminalカメラ状態の最小高 |
| `--theater-tablet-min` | `720px` | tabletシアター全体の検証高 |
| `--theater-mobile-tile-min` | `180px` | mobileカメラタイル最小高 |
| `--theater-mobile-tile-target` | `30dvb` | 固定transportと重ならずmobileで映像を主役にするタイル高 |
| `--theater-event-list-max` | `320px` | tablet event listの最大高 |
| `--theater-representative-max` | `40dvb` | 代表映像の最大表示高 |
| `--theater-mobile-expanded-min` | `60dvb` | mobile expanded映像の最小高 |
| `--app-header-height` | `56px` | desktop/tabletの固定ヘッダー高 |
| `--transport-mobile-max` | `140px` | mobile transportの最大高 |

### Grid

- 作業開始画面は読み幅640pxを上限とし、カードで囲わず左揃えにする。入力、得られる結果、ローカル処理の事実が一画面で読める。
- 完成形のdesktopは216pxライブラリレール、336pxイベント一覧、残りを6カメラシアターへ割り当て、下端に全操作を収めた3段のトランスポートを固定する。
- tabletはカテゴリを上部タブへ移し、イベント一覧を44px以上の開閉操作で折りたためる。mobileは一画面カメラを主とする。
- mobileでは6つのvideo要素を同期とObject URLのライフサイクル維持のためDOMへ残し、選択した一方向だけを表示する。6方向の切り替えはラベル付き横スクロール操作とする。
- 検証幅は375px、768px、1280px。主コンテンツに横スクロールを発生させず、カメラ切り替えだけを局所的に横スクロール可能にする。
- 外側余白は`clamp(var(--space-4), 5vw, var(--space-16))`を用いる。

### Shape

- `--radius-control: 6px`
- `--radius-panel: 8px`
- `--control-min-size: 44px`

## 5. Components

### Primary action

- **Structure**: icon、動詞を先頭にしたbutton label。
- **Variants**: primaryのみ。welcome画面に主操作を複数置かない。
- **Spacing**: `--space-3`と`--space-5`、高さは`--control-min-size`以上。
- **States**: default、hover、active、focus-visible、disabled、loadingを持つ。Task 1はdefault、hover、active、focus-visibleを実装する。
- **Accessibility**: semantic `button`、可視ラベル、3pxの二重focus ring、キーボード到達、44px以上。
- **Motion**: 色と境界の160msのみ。位置や大きさを動かさない。
- **Layout**: inline cluster。狭幅では内容幅に収まり、テキストを途中で切らない。

### Trust statement

- **Structure**: ShieldCheckアイコン、短い見出し、ローカル処理説明。
- **Variants**: standardのみ。
- **Spacing**: `--space-3`、`--space-4`。
- **States**: 常時表示。操作状態は持たない。
- **Accessibility**: アイコンは装飾扱い、文言だけで意味が完結する。
- **Motion**: なし。
- **Layout**: 左揃えのcluster。小画面では本文が自然に折り返す。

### 作業開始画面

- **Structure**: compact product header、入力を示す見出し、primary action、`RecentClips / SavedClips / SentryClips → イベント一覧 / 6カメラ`の結果説明、local-only statement、browser guidance。
- **Variants**: desktop、tablet、mobile。
- **Spacing**: `--space-4`から`--space-20`。
- **States**: emptyが標準。errorとloadingは後続タスクで追加する。
- **Accessibility**: landmark、論理的な見出し順、自然な日本語改行、200%拡大でも情報を失わない。
- **Motion**: なし。
- **Layout**: 左揃えのstack。背景装飾、中央のrounded panel、英語の大文字eyebrowを置かない。

### Source summary

- **Structure**: 選択したフォルダ名、総イベント件数、読み込み警告、再選択操作。
- **States**: standard、warning。warningだけを色とアイコンで強調する。
- **Accessibility**: フォルダ名は省略表示してもtitle相当の全文を読み取れる。再選択は可視ラベルを持つ。
- **Layout**: desktopでは左レール上部、tablet/mobileではheader直下に置く。

### Event row

- **Structure**: category、local timestamp、duration、segment count、`n/6 cameras`、欠損方向。
- **States**: default、hover、focus-visible、selected、partial、no-poster。
- **Copy**: 正常時は`6/6 カメラ`と肯定形で示す。欠損時だけ`5/6 カメラ・右リピーターなし`のように方向を示す。`0台のカメラがありません`は使わない。
- **Thumbnail**: 実際のposterがある場合だけ表示する。posterがない健康なイベントに空の画像枠やエラーアイコンを置かない。
- **Layout**: cardを並べず、一覧面の中をdividerとselected barで区切る。

### Theater context

- **Structure**: source folder、category、local timestamp、event duration、segment count、camera availability。
- **States**: complete、partial、terminal fallback。
- **Accessibility**: 色だけに依存せず、シアター単体のスクリーンショットでも現在の証拠を特定できる。
- **Layout**: camera gridの直上に常設し、mobileでもイベント一覧を読み戻さず確認できる。

### 6-camera theater

- **Structure**: canonical orderの6タイル、共有timeline、transport、速度、前後イベント操作。
- **Variants**: 2×3 grid、単一カメラexpanded、6方向すべてterminal時だけ代表映像。
- **States**: loading、ready、missing、unavailable、corrupt、unsupportedを各タイルで独立表示する。
- **Terminal fallback**: 代表映像を併記するときはカメラ名とterminal状態を80pxタイルの左右へ分け、互いに重ねず、6状態・代表映像・transportを別領域に保つ。
- **Accessibility**: 各タイルは日本語カメラ名と拡大状態を持ち、transportは可視ラベル、44px操作、range label、キーボードshortcutsを備える。
- **Motion**: 状態色とborderの`--motion-fast`のみ。expanded切替で位置やサイズをアニメーションしない。
- **Layout**: desktopは2×3、tablet/mobileは縦方向へ再配置し、375pxで横overflowを生じさせない。
- **Persistence**: transportはdesktopでシアター下端にsticky、tablet/mobileではviewport下端に固定し、映像をスクロール中も操作可能にする。

### Camera switcher

- **Structure**: canonical orderの6方向ラベルと現在選択状態。
- **States**: mobileで常時表示し、選択中は`aria-pressed`とaccent境界を併用する。desktop/tabletでは6画面シアターを優先して非表示にする。
- **Accessibility**: 「表示するカメラ」というgroup名、方向ごとの可視ラベル、44px以上の操作面、明確なfocus ringを持つ。
- **Motion**: 色とborderだけを`--motion-fast`で遷移し、reduced motionでは0msにする。
- **Layout**: 375pxでは一行横スクロールとし、ページ全体の横overflowへ波及させない。

### Event list disclosure

- **Structure**: 件数と開閉buttonをイベント見出しへまとめる。
- **States**: 1024px以下で表示し、開閉を`aria-expanded`と矢印方向で示す。desktopではイベント一覧を常時表示する。
- **Accessibility**: `aria-controls`で一覧と関連付け、44px以上の操作面とfocus ringを持つ。
- **Motion**: 状態理解に不要な開閉アニメーションは行わない。

### Compatibility status

- `showDirectoryPicker`がない環境では、互換モードでOSのフォルダ選択を使うことをwelcome上へ常時表示する。
- primary actionの直接操作からdirectory inputを開き、ブラウザのuser activationを保持する。
- 未対応を失敗と表現せず、Chrome/Edgeの標準選択とdirectory inputの違いを正確に案内する。

### Playback transport

- **Structure**: 前へ、-10秒、再生/一時停止、+10秒、次へ、elapsed/duration、range、速度選択。
- **States**: default、hover、active、focus-visible、disabled、recoverable play error。
- **Accessibility**: iconと日本語ラベルを併用し、検知位置はrange上の非操作markerとして名前を持つ。
- **Mobile layout**: rangeを一段、再生と前後移動を一段にまとめ、defaultで`--transport-mobile-max`以下にする。速度はラベル付きselectで保持し、映像を45dvb以上確保する。

## 6. Motion & Interaction

### Timing

| Token | Duration | Easing | Usage |
|---|---:|---|---|
| `--motion-fast` | `160ms` | `cubic-bezier(0.2, 0, 0, 1)` | hover、active、focusの状態遷移 |

### Rules

- 状態理解に必要な色、border-color、opacityだけを160msで遷移する。
- レイアウト、位置、サイズをアニメーションしない。
- `prefers-reduced-motion: reduce`ではtransition durationを0msにする。
- hoverだけに意味を置かず、focus-visibleとactiveでも同等の状態を伝える。

## 7. Depth & Surface

戦略はtonal-shiftを主とし、操作境界とfault stateにだけ1px borderを併用する。box-shadow、blur、glow、gradientは使わない。Canvas、Rail、Surface、Surface raisedの順にわずかに明るくし、映像を置く領域は周辺より暗く保つ。6pxは操作、8pxはシアター外枠に限定する。通常のイベント行、ラベル、メタデータをrounded outlineで囲わない。永続的なelevationはシアターと選択イベントの2段階までとする。

## 8. Accessibility Constraints & Accepted Debt

### Constraints

- WCAG 2.2 AAを目標とし、通常文字4.5:1、大文字または24px以上の文字とUI境界3:1を下限とする。
- 全操作をキーボードで到達可能にし、focus-visibleを背景から明確に識別できる二重ringで示す。
- タップ領域は44×44px以上。色に加えて文言とアイコンでカテゴリ、警告、状態を伝える。
- 200%拡大、375px幅、日本語の長いラベル、reduced motion、Windows high contrastを考慮する。
- 走査進捗と再生エラーは後続実装でlive regionへ通知する。

### Accepted Debt

| Item | Location | Why accepted | Owner / Exit |
|---|---|---|---|
| VoiceOver実機確認 | viewer shell | 自動検証とキーボード確認までを継続する | 公開後の実SSD確認時に読み順と操作を確認 |
