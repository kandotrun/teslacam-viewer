/** States of a camera window (DESIGN.md §6 Camera window). */
export type CameraStatus = 'loading' | 'ready' | 'missing' | 'unavailable' | 'corrupt' | 'unsupported'

export const terminalStatuses: readonly CameraStatus[] = ['missing', 'unavailable', 'corrupt', 'unsupported']

export function cameraStatusLabel(status: CameraStatus): string {
  switch (status) {
    case 'loading':
      return '読み込み中'
    case 'ready':
      return '再生できます'
    case 'missing':
      return '記録がありません'
    case 'unavailable':
      return '映像を読み込めません'
    case 'corrupt':
      return '映像が破損しています'
    case 'unsupported':
      return '未対応の映像形式です'
  }
}

/** Maps a media element error code; aborts while a source is being replaced are ignored (null). */
export function mediaErrorStatus(code: number | undefined, replacing: boolean): CameraStatus | null {
  switch (code) {
    case 3:
      return 'corrupt'
    case 4:
      return 'unsupported'
    case 1:
      return replacing ? null : 'unavailable'
    default:
      return 'unavailable'
  }
}
