# TeslaCam Viewer Design System

## 0. Research Log

- Embedded refs: Tesla、Sentry、Runwayを候補化し、映像を主役にする静けさと精密な操作感を両立できるため、Layer Aはminimalist、Layer BはTeslaを採用した。ブランドロゴ、固有書体、公式UIは複製しない。
- Lazyweb: `video review workspace`、`security camera playback timeline`、`cinematic media library`の3クエリを実行し、Frame.io、Vimeo、Milestone XProtect、Soraの4画面を閲覧した。左のライブラリレール、時系列イベント列、主映像を支配的に置くシアター、画面下部に固定するトランスポートというレイアウト文法だけを抽出した。
- Imagen drafts: `exec-6d68dff4-c919-40dc-91e4-e86a587fda7c.png`、`exec-5cc54840-df75-4158-a0c4-9c38f47b1218.png`、`exec-053c7f42-d7cf-4d17-82c1-614fd8199a96.png`を比較し、1案目の`exec-6d68dff4-c919-40dc-91e4-e86a587fda7c.png`を参照契約として採用した。
- Target composition: チャコールのライブラリレール、イベント一覧、6カメラシアター、永続トランスポートバー。初期実装では同じトークンと状態規則によるwelcome shellだけを構築する。

## 1. Atmosphere & Identity

暗い編集室で、映像だけが静かに立ち上がるローカル映像ワークスペース。印象の核は、黒に近い面をわずかな明度差で積層する「チャコール・シアター」である。高彩度色は主操作とカテゴリ状態に限定し、Tesla由来の余白、抑制した文字階層、単一CTAの精度を、TeslaCam確認という独自用途へ再構成する。想定利用者は、SSDをMacへつないですぐ確認したい所有者、事故やセントリー記録を時刻から探す調査者、欠損カメラを含む大量イベントを長時間確認する運用者である。

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
| Border strong | `--color-border-strong` | `#46515c` | 強調境界 |
| Accent | `--color-accent` | `#4f7df3` | 唯一の主操作、focus |
| Accent hover | `--color-accent-hover` | `#6a91f6` | 主操作hover |
| Accent active | `--color-accent-active` | `#3f69d4` | 主操作active |
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
| Display | `--font-display` | `clamp(2.25rem, 5vw, 4rem)` | 560 | 1.05 | `-0.035em` | welcomeの主見出し |
| Heading | `--font-heading` | `1.5rem` | 560 | 1.25 | `-0.02em` | パネル見出し |
| Body large | `--font-body-large` | `1.125rem` | 400 | 1.65 | `-0.01em` | リード文 |
| Body | `--font-body` | `1rem` | 400 | 1.6 | `0` | 標準本文、操作 |
| Small | `--font-small` | `0.875rem` | 450 | 1.5 | `0` | ブラウザ案内 |
| Label | `--font-label` | `0.75rem` | 600 | 1.35 | `0.08em` | カテゴリ、メタデータ |

### Font Stack

- Primary: `ui-sans-serif, -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Yu Gothic", sans-serif`
- Mono: `ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace`
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

### Grid

- welcome shellは中央の読み幅720pxを上限とし、上下はviewportに対して中央配置する。
- 完成形のdesktopは224pxライブラリレール、320pxイベント一覧、残りを6カメラシアターへ割り当て、下端に72pxのトランスポートを固定する。
- tabletはカテゴリを上部タブへ移し、イベント一覧を開閉可能にする。mobileは一画面カメラを主とする。
- 検証幅は375px、768px、1280px。主コンテンツに横スクロールを発生させない。
- 外側余白は`clamp(var(--space-4), 5vw, var(--space-16))`を用いる。

### Shape

- `--radius-control: 12px`
- `--radius-panel: 16px`
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

### Welcome shell

- **Structure**: `main`内にeyebrow、見出し、説明、primary action、trust statement、browser guidance。
- **Variants**: desktop、tablet、mobile。
- **Spacing**: `--space-4`から`--space-20`。
- **States**: emptyが標準。errorとloadingは後続タスクで追加する。
- **Accessibility**: landmark、論理的な見出し順、自然な日本語改行、200%拡大でも情報を失わない。
- **Motion**: なし。
- **Layout**: centered stack。背景の二本の直線と小さなカメラ位置記号で、将来の6カメラシアターを抽象的に示す。

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

戦略はtonal-shiftを主とし、操作境界に1px borderだけを併用する。box-shadow、blur、glow、gradientは使わない。Canvas、Rail、Surface、Surface raisedの順にわずかに明るくし、映像を置く領域は周辺より暗く保つ。12pxは操作、16pxは主要パネルに限定し、丸いカードを入れ子にしない。

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
| 実フォルダ選択のブラウザ能力判定とfallback | welcome shell | Task 1は実行基盤と主操作の表示までが範囲 | Task 5でFile System Access APIとdirectory inputを接続するときに解消 |
| 完成形4ペインのprimitive showcase | viewer shell | Task 1ではwelcome shellだけが承認範囲 | Task 6でlibrary、event list、theater、transportを実装する前に検証 |
| スクリーンリーダー実機確認 | welcome shell | 自動検証とキーボード確認までをTask 1で行う | E2E Task 9でVoiceOverによる読み順と操作を確認 |
