/**
 * Typed, validated view of the shared design contract (`packages/contract`). Both JSON files are parsed
 * once at start-up; a malformed file fails loudly instead of rendering with silently wrong values.
 */
import { z } from 'zod'

import compositeLayoutJson from '../../../../../packages/contract/composite-layout.json'
import designTokensJson from '../../../../../packages/contract/design-tokens.json'
import type { CameraId } from '../../teslacam/types'

const hexColor = z.string().regex(/^#[0-9A-Fa-f]{6}(?:[0-9A-Fa-f]{2})?$/u)
const pixels = z.number().nonnegative()
const fraction = z.number().min(0).max(1)

const rectSchema = z.object({ x: pixels, y: pixels, width: pixels, height: pixels }).strict()

function perCamera<T extends z.ZodType>(schema: T) {
  return z.object({
    front: schema,
    back: schema,
    left_pillar: schema,
    left_repeater: schema,
    right_pillar: schema,
    right_repeater: schema,
  }).strict()
}

const shadowLayerSchema = z.object({ x: z.number(), y: z.number(), blur: pixels, color: hexColor }).strict()
const typeRoleSchema = z.object({
  family: z.enum(['ui', 'num']),
  weight: z.number().int(),
  size: pixels,
  lineHeight: z.number().positive(),
}).strict()
const motionSchema = z.object({
  durationMs: pixels,
  easing: z.tuple([z.number(), z.number(), z.number(), z.number()]),
}).strict()

const designTokensSchema = z.object({
  $comment: z.string(),
  color: z.record(z.string().regex(/^[a-z][a-z0-9-]*$/u), hexColor),
  shadow: z.record(z.string(), z.array(shadowLayerSchema).min(1)),
  radius: z.record(z.string(), pixels),
  font: z.object({ ui: z.array(z.string()).min(1), num: z.array(z.string()).min(1) }).strict(),
  type: z.record(z.string(), typeRoleSchema),
  space: z.object({ unit: pixels, gutter: pixels }).strict(),
  size: z.record(z.string(), pixels),
  motion: z.object({ fast: motionSchema, sheet: motionSchema, 'reveal-stagger-ms': pixels }).strict(),
  breakpoint: z.object({ 'phone-max': pixels, 'tablet-max': pixels, 'compact-library-max': pixels }).strict(),
  stage: z.object({
    areas: z.array(z.array(z.string()).length(3)).length(3),
    rows: z.array(z.number().positive()).length(3),
    columns: z.array(z.number().positive()).length(3),
    gap: z.object({ row: pixels, column: pixels }).strict(),
    'back-width-ratio': fraction,
    mounts: perCamera(z.tuple([fraction, fraction])),
  }).strict(),
  contrast: z.array(z.object({ fg: z.string(), bg: z.string(), min: z.number().positive() }).strict()),
}).strict()

const compositeLayoutSchema = z.object({
  $comment: z.string(),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  fps: z.number().int().positive(),
  background: z.string(),
  slots: perCamera(rectSchema),
  vehicle: rectSchema,
  videoCornerRadius: pixels,
  label: z.object({
    inset: pixels,
    height: pixels,
    paddingInline: pixels,
    fontSize: pixels,
    fontWeight: z.number().int(),
    scrim: z.string(),
    text: z.string(),
  }).strict(),
  missing: z.object({ fill: z.string(), text: z.string(), message: z.string().min(1) }).strict(),
  wedges: z.object({ color: z.string(), mountsFrom: z.string() }).strict(),
  labels: perCamera(z.string().min(1)),
}).strict()

export type DesignTokens = z.infer<typeof designTokensSchema>
export type CompositeLayoutContract = z.infer<typeof compositeLayoutSchema>
export type ContractRect = z.infer<typeof rectSchema>
export type MountPoint = readonly [number, number]

export const designTokens: DesignTokens = designTokensSchema.parse(designTokensJson)
export const compositeLayoutContract: CompositeLayoutContract = compositeLayoutSchema.parse(compositeLayoutJson)

/** A colour token value (`#RRGGBB` or `#RRGGBBAA`); unknown names fail at the call site's test. */
export function colorToken(name: string): string {
  const value = designTokens.color[name]
  if (value === undefined) throw new Error(`Unknown colour token: ${name}`)
  return value
}

export function fontStack(family: 'ui' | 'num'): string {
  return designTokens.font[family]
    .map((name) => (/^[a-z-]+$/u.test(name) ? name : `"${name}"`))
    .join(', ')
}

export const cameraMounts: Readonly<Record<CameraId, MountPoint>> = designTokens.stage.mounts
