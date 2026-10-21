import { useState } from 'react'

import { EventList } from './EventList'
import { events } from './EventList.test-fixtures'

export function EventListHarness() {
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null)
  return (
    <>
      <EventList events={events} selectedEventId={selectedEventId} onSelectEvent={setSelectedEventId} />
      <output>{selectedEventId ?? '未選択'}</output>
    </>
  )
}
