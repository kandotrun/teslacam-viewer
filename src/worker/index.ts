import { Hono } from 'hono'

const securityHeaders = {
  'Content-Security-Policy':
    "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
  'Permissions-Policy':
    'camera=(), microphone=(), geolocation=(), payment=(), usb=()',
  'Referrer-Policy': 'no-referrer',
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
} as const

const app = new Hono<{ Bindings: CloudflareBindings }>()

app.use('*', async (context, next) => {
  await next()

  for (const [name, value] of Object.entries(securityHeaders)) {
    context.header(name, value)
  }
})

const routes = app.get('/api/health', (context) => {
  return context.json({ status: 'ok' as const })
})

export type AppType = typeof routes
export default app
