// 结构化动作配置表单（按 manifest inputs schema 渲染）
// Schema-driven action config form (renders per manifest inputs schema)
import { useId } from 'react'
import { type FieldSpec, type FieldOption } from '@/types'

// ============================================================
// Props
// ============================================================

export interface ActionConfigFormProps {
  /** 当前 config 值 */
  config: Record<string, unknown>
  /** manifest.actions[].inputs schema */
  inputs: Record<string, FieldSpec> | undefined
  /** config 变更回调 */
  onChange: (config: Record<string, unknown>) => void
  /** 禁用态 */
  disabled?: boolean
}

// ============================================================
// 单字段渲染器
// Single field renderer
// ============================================================

function StringField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: string) => void
  disabled?: boolean
}) {
  const id = useId()
  return (
    <div className="space-y-0.5">
      <label htmlFor={id} className="text-[10px] text-muted-foreground">
        {spec.label ?? id}
        {spec.required && <span className="ml-0.5 text-destructive">*</span>}
      </label>
      <input
        id={id}
        type="text"
        className="h-7 w-full rounded-md border bg-background px-2 text-xs"
        value={(value as string) ?? ''}
        onChange={(e) => onChange(e.target.value)}
        placeholder={spec.placeholder ?? spec.default as string ?? ''}
        minLength={spec.minLength}
        maxLength={spec.maxLength}
        disabled={disabled}
      />
    </div>
  )
}

function NumberField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: number) => void
  disabled?: boolean
}) {
  const id = useId()
  return (
    <div className="space-y-0.5">
      <label htmlFor={id} className="text-[10px] text-muted-foreground">
        {spec.label ?? id}
        {spec.required && <span className="ml-0.5 text-destructive">*</span>}
      </label>
      <input
        id={id}
        type="number"
        className="h-7 w-full rounded-md border bg-background px-2 text-xs"
        value={value as number ?? (spec.default as number) ?? ''}
        onChange={(e) => onChange(e.target.value === '' ? 0 : Number(e.target.value))}
        min={spec.min}
        max={spec.max}
        placeholder={spec.placeholder ?? (spec.default != null ? String(spec.default) : '')}
        disabled={disabled}
      />
    </div>
  )
}

function BooleanField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: boolean) => void
  disabled?: boolean
}) {
  const id = useId()
  return (
    <div className="flex items-center gap-2">
      <input
        id={id}
        type="checkbox"
        className="size-3.5 accent-primary"
        checked={Boolean(value ?? spec.default ?? false)}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
      />
      <label htmlFor={id} className="cursor-pointer text-[10px] text-muted-foreground">
        {spec.label ?? id}
      </label>
    </div>
  )
}

function EnumField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: string) => void
  disabled?: boolean
}) {
  const id = useId()
  const options: FieldOption[] = spec.options ?? []
  const current = value ?? spec.default ?? (options[0]?.value ?? '')

  return (
    <div className="space-y-0.5">
      <label htmlFor={id} className="text-[10px] text-muted-foreground">
        {spec.label ?? id}
        {spec.required && <span className="ml-0.5 text-destructive">*</span>}
      </label>
      <select
        id={id}
        className="h-7 w-full rounded-md border bg-background px-2 text-xs"
        value={current as string}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
      >
        {options.length === 0 && <option value="">—</option>}
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  )
}

function FileField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: string) => void
  disabled?: boolean
}) {
  const id = useId()
  return (
    <div className="space-y-0.5">
      <label htmlFor={id} className="text-[10px] text-muted-foreground">
        {spec.label ?? id}
        {spec.required && <span className="ml-0.5 text-destructive">*</span>}
      </label>
      <input
        id={id}
        type="text"
        className="h-7 w-full rounded-md border bg-background px-2 text-xs"
        value={(value as string) ?? ''}
        onChange={(e) => onChange(e.target.value)}
        placeholder={spec.placeholder ?? spec.default as string ?? 'file path…'}
        disabled={disabled}
      />
    </div>
  )
}

function JsonField({
  spec,
  value,
  onChange,
  disabled,
}: {
  spec: FieldSpec
  value: unknown
  onChange: (v: unknown) => void
  disabled?: boolean
}) {
  const id = useId()
  const text = value != null ? JSON.stringify(value, null, 2) : ''

  return (
    <div className="space-y-0.5">
      <label htmlFor={id} className="text-[10px] text-muted-foreground">
        {spec.label ?? id}
        {spec.required && <span className="ml-0.5 text-destructive">*</span>}
      </label>
      <textarea
        id={id}
        className="h-16 w-full rounded-md border bg-background px-2 py-1 font-mono text-[10px]"
        value={text}
        onChange={(e) => {
          try {
            onChange(e.target.value ? JSON.parse(e.target.value) : undefined)
          } catch {
            // invalid JSON — keep in textarea but don't push to config yet
          }
        }}
        placeholder='{ "key": "value" }'
        disabled={disabled}
        spellCheck={false}
      />
    </div>
  )
}

// ============================================================
// 主组件
// ============================================================

export function ActionConfigForm({ config, inputs, onChange, disabled }: ActionConfigFormProps) {
  // 无 schema 时显示空状态
  // Empty state when the action declares no inputs
  if (!inputs || Object.keys(inputs).length === 0) {
    return (
      <p className="text-[10px] italic text-muted-foreground">
        此动作无输入参数 / No input parameters
      </p>
    )
  }

  return (
    <div className="space-y-2">
      {Object.entries(inputs).map(([key, typedSpec]) => {
        const value = config[key] ?? typedSpec.default

        const set = (v: unknown) => {
          if (v === undefined) {
            const next = { ...config }
            delete next[key]
            onChange(next)
          } else {
            onChange({ ...config, [key]: v })
          }
        }

        switch (typedSpec.type) {
          case 'string':
            return <StringField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
          case 'number':
            return <NumberField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
          case 'boolean':
            return <BooleanField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
          case 'enum':
            return <EnumField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
          case 'file':
            return <FileField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
          case 'json':
          default:
            return <JsonField key={key} spec={typedSpec} value={value} onChange={set} disabled={disabled} />
        }
      })}
    </div>
  )
}
