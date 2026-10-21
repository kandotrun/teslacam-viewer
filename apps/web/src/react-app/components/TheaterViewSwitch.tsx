export type TheaterViewMode = 'spatial' | 'grid'

type TheaterViewSwitchProps = {
  readonly mode: TheaterViewMode
  readonly disabled: boolean
  readonly onChange: (mode: TheaterViewMode) => void
}

export function TheaterViewSwitch({ mode, disabled, onChange }: TheaterViewSwitchProps) {
  return (
    <div className="theater-view-switch" role="group" aria-label="表示レイアウト">
      <button
        aria-pressed={mode === 'spatial'}
        disabled={disabled}
        type="button"
        onClick={() => onChange('spatial')}
      >
        車両配置
      </button>
      <button
        aria-pressed={mode === 'grid'}
        disabled={disabled}
        type="button"
        onClick={() => onChange('grid')}
      >
        6分割
      </button>
    </div>
  )
}
