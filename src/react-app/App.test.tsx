import { render, screen } from '@testing-library/react'

import { App } from './App'

describe('welcome view', () => {
  it('shows the folder selection action', () => {
    render(<App />)

    expect(
      screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }),
    ).toBeVisible()
  })

  it('explains that videos stay on this computer', () => {
    render(<App />)

    expect(screen.getByText(/外部へ送信されません/)).toBeVisible()
  })

  it('guides users to a supported desktop browser', () => {
    render(<App />)

    expect(screen.getByText(/ChromeまたはEdge/)).toBeVisible()
  })
})
