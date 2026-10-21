import { Fragment } from 'react'

/** Sets digit runs in Barlow (`.num`) and leaves Japanese units in the UI font: `2分` → 2 + 分. */
export function NumeralText({ text }: { readonly text: string }) {
  const parts = text.split(/([0-9][0-9:,./–]*)/u).filter((part) => part.length > 0)
  return (
    <>
      {parts.map((part, index) => (/^[0-9]/u.test(part)
        ? <span className="num" key={index}>{part}</span>
        : <Fragment key={index}>{part}</Fragment>))}
    </>
  )
}
