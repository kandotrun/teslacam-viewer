/**
 * Zod configured for the deployed Content-Security-Policy (`script-src 'self'`, no `unsafe-eval`):
 * jitless parsing never probes `new Function`, which would be reported as a CSP violation.
 */
import { z } from 'zod'

z.config({ jitless: true })

export { z }
