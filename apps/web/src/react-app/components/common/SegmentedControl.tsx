import { useRef, type KeyboardEvent, type ReactNode } from 'react'

export type SegmentedOption<T extends string> = {
  readonly value: T
  readonly label: ReactNode
  /** Full accessible name (the visible label may be shortened). */
  readonly name: string
  readonly icon?: ReactNode
  readonly disabled?: boolean
}

type SegmentedControlProps<T extends string> = {
  readonly label: string
  readonly options: readonly SegmentedOption<T>[]
  readonly value: T
  readonly onChange: (value: T) => void
  readonly className: string
}

const nextKeys = new Set(['ArrowRight', 'ArrowDown'])
const previousKeys = new Set(['ArrowLeft', 'ArrowUp'])

/** A radio group drawn as a segmented track (DESIGN.md §9: segmented controls are radio groups). */
export function SegmentedControl<T extends string>({ label, options, value, onChange, className }: SegmentedControlProps<T>) {
  const buttons = useRef(new Map<T, HTMLButtonElement>())
  const enabled = options.filter((option) => option.disabled !== true)

  function move(event: KeyboardEvent<HTMLButtonElement>, current: T): void {
    const index = enabled.findIndex((option) => option.value === current)
    let target: SegmentedOption<T> | undefined
    if (nextKeys.has(event.key)) target = enabled[(index + 1) % enabled.length]
    else if (previousKeys.has(event.key)) target = enabled[(index - 1 + enabled.length) % enabled.length]
    else if (event.key === 'Home') target = enabled[0]
    else if (event.key === 'End') target = enabled.at(-1)
    if (target === undefined) return
    event.preventDefault()
    onChange(target.value)
    buttons.current.get(target.value)?.focus()
  }

  return (
    <div aria-label={label} className={className} role="radiogroup">
      {options.map((option) => {
        const checked = option.value === value
        return (
          <button
            aria-checked={checked}
            aria-label={option.name}
            disabled={option.disabled}
            key={option.value}
            ref={(button) => {
              if (button === null) buttons.current.delete(option.value)
              else buttons.current.set(option.value, button)
            }}
            role="radio"
            tabIndex={checked ? 0 : -1}
            type="button"
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => move(event, option.value)}
          >
            {option.icon}
            <span className="segmented__label">{option.label}</span>
          </button>
        )
      })}
    </div>
  )
}
