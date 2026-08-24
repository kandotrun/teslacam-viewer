import app from './index'

describe('health route', () => {
  it('returns a typed health response for GET', async () => {
    const response = await app.request('/api/health')

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({ status: 'ok' })
  })

  it('returns an empty successful response for HEAD', async () => {
    const response = await app.request('/api/health', { method: 'HEAD' })

    expect(response.status).toBe(200)
    await expect(response.text()).resolves.toBe('')
  })

  it('returns the API security policy', async () => {
    const response = await app.request('/api/health')

    expect(response.headers.get('Content-Security-Policy')).toBe(
      "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
    )
    expect(response.headers.get('X-Frame-Options')).toBe('DENY')
    expect(response.headers.get('X-Content-Type-Options')).toBe('nosniff')
    expect(response.headers.get('Referrer-Policy')).toBe('no-referrer')
    expect(response.headers.get('Permissions-Policy')).toBe(
      'camera=(), microphone=(), geolocation=(), payment=(), usb=()',
    )
  })
})

describe('API boundary', () => {
  it('returns a structured JSON response for the exact API root', async () => {
    const response = await app.request('/api')

    expect(response.status).toBe(404)
    expect(response.headers.get('Content-Type')).toContain('application/json')
    await expect(response.json()).resolves.toEqual({ error: 'not_found' })
  })

  it('returns a structured JSON response for an unknown API route', async () => {
    const response = await app.request('/api/missing')

    expect(response.status).toBe(404)
    expect(response.headers.get('Content-Type')).toContain('application/json')
    await expect(response.json()).resolves.toEqual({ error: 'not_found' })
  })

  it('returns a structured JSON response and Allow header for unsupported health methods', async () => {
    const response = await app.request('/api/health', { method: 'POST' })

    expect(response.status).toBe(405)
    expect(response.headers.get('Allow')).toBe('GET, HEAD')
    await expect(response.json()).resolves.toEqual({ error: 'method_not_allowed' })
  })

  it('does not emit CORS headers from the API boundary', async () => {
    const response = await app.request('/api/health')

    expect(response.headers.get('Access-Control-Allow-Origin')).toBeNull()
  })
})
