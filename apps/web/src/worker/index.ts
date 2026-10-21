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

app.use('/api/health', async (context, next) => {
  if (context.req.method !== 'GET' && context.req.method !== 'HEAD') {
    context.header('Allow', 'GET, HEAD')
    return context.json({ error: 'method_not_allowed' as const }, 405)
  }

  await next()
})

const routes = app.get('/api/health', (context) => {
  return context.json({ status: 'ok' as const })
})

app.notFound((context) => {
  if (context.req.path === '/api' || context.req.path.startsWith('/api/')) {
    return context.json({ error: 'not_found' as const }, 404)
  }

  return context.text('Not Found', 404)
})

export type AppType = typeof routes
export default app
