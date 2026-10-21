import { useState } from 'react'

import { Theater as TheaterComponent, type TheaterProps } from './Theater'
import type { TheaterViewMode } from './TheaterViewSwitch'

type TheaterHarnessProps = Omit<TheaterProps, 'viewMode' | 'onViewMode'>

export function Theater(props: TheaterHarnessProps) {
  const [viewMode, setViewMode] = useState<TheaterViewMode>('spatial')
  return <TheaterComponent {...props} viewMode={viewMode} onViewMode={setViewMode} />
}
