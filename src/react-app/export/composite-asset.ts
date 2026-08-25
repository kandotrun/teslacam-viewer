import { CompositeVideoExportError, exportAborted } from './composite-recorder'

export type CompositeVehicleAssetDependencies = {
  readonly createImage: () => HTMLImageElement
}

const vehicleAssetPath = '/vehicle-top.png'

function assetLoadFailed(): CompositeVideoExportError {
  return new CompositeVideoExportError('recording-failed', '車両イメージを読み込めませんでした。')
}

export function loadCompositeVehicleImage(
  dependencies: CompositeVehicleAssetDependencies,
  signal: AbortSignal | undefined,
): Promise<HTMLImageElement> {
  const image = dependencies.createImage()
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      image.onload = null
      image.onerror = null
      signal?.removeEventListener('abort', abort)
    }
    const complete = () => {
      cleanup()
      resolve(image)
    }
    const fail = (error: CompositeVideoExportError) => {
      cleanup()
      reject(error)
    }
    const abort = () => {
      cleanup()
      image.src = ''
      reject(exportAborted())
    }
    image.onload = complete
    image.onerror = () => fail(assetLoadFailed())
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted === true) {
      abort()
      return
    }
    image.src = vehicleAssetPath
    if (image.complete && image.naturalWidth > 0) complete()
  })
}
