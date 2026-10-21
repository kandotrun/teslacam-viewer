// @vitest-environment node
import { describe } from 'vitest'

import { parseClipFileName } from '../teslacam/filename'
import { cameraIds } from '../teslacam/types'
import { defineFixture, type FixtureCase } from './fixture-file'

function parseCase(name: string, fileName: string): FixtureCase {
  return {
    name,
    fn: 'parseClipFileName',
    input: { name: fileName },
    expected: parseClipFileName(fileName),
  }
}

describe('contract: clip-filename', () => {
  defineFixture({
    file: 'clip-filename',
    module: 'filename',
    source: ['apps/web/src/teslacam/filename.ts'],
    cases: () => [
      ...cameraIds.map((camera) => parseCase(`camera ${camera}`, `2026-08-21_14-30-45-${camera}.mp4`)),
      parseCase('representative helper', 'event.mp4'),
      parseCase('thumbnail helper', 'thumb.png'),
      parseCase('metadata helper', 'event.json'),
      parseCase('unknown camera cabin', '2026-08-21_14-30-45-cabin.mp4'),
      parseCase('camera with digits', '2026-08-21_14-30-45-front2.mp4'),
      parseCase('camera with hyphen', '2026-08-21_14-30-45-left-pillar.mp4'),
      parseCase('empty camera', '2026-08-21_14-30-45-.mp4'),
      parseCase('upper-case extension', '2026-08-21_14-30-45-front.MP4'),
      parseCase('upper-case camera', '2026-08-21_14-30-45-Front.mp4'),
      parseCase('other extension', '2026-08-21_14-30-45-front.mov'),
      parseCase('double extension', '2026-08-21_14-30-45-front.mp4.mp4'),
      parseCase('trailing newline', '2026-08-21_14-30-45-front.mp4\n'),
      parseCase('leading space', ' 2026-08-21_14-30-45-front.mp4'),
      parseCase('AppleDouble prefix', '._2026-08-21_14-30-45-front.mp4'),
      parseCase('path separator', 'RecentClips/2026-08-21_14-30-45-front.mp4'),
      parseCase('not a timestamp', 'not-a-timestamp-front.mp4'),
      parseCase('non-leap February 29', '2026-02-29_14-30-45-front.mp4'),
      parseCase('leap day 2028', '2028-02-29_14-30-45-front.mp4'),
      parseCase('century non-leap 2100', '2100-02-29_00-00-00-front.mp4'),
      parseCase('quad-century leap 2000', '2000-02-29_00-00-00-front.mp4'),
      parseCase('hour 24', '2026-08-21_24-30-45-front.mp4'),
      parseCase('minute 60', '2026-08-21_14-60-45-front.mp4'),
      parseCase('second 60', '2026-08-21_14-30-60-front.mp4'),
      parseCase('month 00', '2026-00-21_14-30-45-front.mp4'),
      parseCase('month 13', '2026-13-21_14-30-45-front.mp4'),
      parseCase('day 00', '2026-08-00_14-30-45-front.mp4'),
      parseCase('day 31 in a 30-day month', '2026-09-31_14-30-45-front.mp4'),
      parseCase('day 31 in a 31-day month', '2026-08-31_23-59-59-back.mp4'),
      parseCase('year 0000', '0000-01-01_00-00-00-front.mp4'),
      parseCase('year 0001', '0001-01-01_00-00-00-front.mp4'),
      parseCase('year 9999', '9999-12-31_23-59-59-right_repeater.mp4'),
      parseCase('five-digit year', '12026-08-21_14-30-45-front.mp4'),
      parseCase('single-digit month', '2026-8-21_14-30-45-front.mp4'),
      parseCase('metadata separators', '2026-08-21T14:30:45-front.mp4'),
      parseCase('full-width digits', '２０２６-08-21_14-30-45-front.mp4'),
      parseCase('Arabic-Indic digits', '٢٠٢٦-08-21_14-30-45-front.mp4'),
      parseCase('empty name', ''),
    ],
  })
})
