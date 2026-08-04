import {
  createContext,
  useContext,
  useEffect,
  useId,
  useMemo,
  useState,
} from "react"
import type React from "react"
import {
  AlertCircle,
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Eye,
  EyeOff,
  KeyRound,
  Minus,
  Plus,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react"

import type { JsonValue } from "@/lib/api"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { Button, type ButtonProps } from "@/components/ui/button"
import { Input, Textarea } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import {
  applySchemaDefaults,
  arrayItemSchema,
  asSchema,
  childErrorCount,
  cloneJson,
  defaultArrayItem,
  emptyValue,
  enumKey,
  enumOptions,
  errorsForPointer,
  getAtPointer,
  humanizeProperty,
  isJsonObject,
  isNullableSchema,
  isPlainObject,
  isSecretSchema,
  joinPointer,
  materializeSchema,
  matchingVariantIndex,
  numberKeyword,
  orderedProperties,
  propertySchema,
  remapSecretsAfterArrayOperation,
  removeAtPointer,
  schemaType,
  schemaVariants,
  setAtPointer,
  uiOptionsFor,
  type FormValidationError,
  type JsonObject,
  type JsonSchema,
  type SecretDraft,
  type UiOptions,
} from "./schema-utils"

interface SchemaFormProps {
  schema: JsonSchema
  uiSchema: JsonSchema
  value: JsonObject
  errors: FormValidationError[]
  secrets: SecretDraft
  disabled?: boolean
  onChange: (value: JsonObject) => void
  onSecretsChange: (secrets: SecretDraft) => void
}

interface FormContextValue extends SchemaFormProps {
  variants: Record<string, number>
  setVariant: (pointer: string, index: number) => void
  update: (pointer: string, value: JsonValue) => void
  remove: (pointer: string) => void
}

const FormContext = createContext<FormContextValue | null>(null)

export function SchemaForm(props: SchemaFormProps) {
  const [variants, setVariants] = useState<Record<string, number>>({})
  const rootSchema = materializeSchema(props.schema, props.schema, props.value)
  const rootUi = uiOptionsFor(props.uiSchema, "", rootSchema)
  const rootType = schemaType(rootSchema, props.value)
  const rootErrors = errorsForPointer(props.errors, "")

  const context = useMemo<FormContextValue>(
    () => ({
      ...props,
      variants,
      setVariant: (pointer, index) => setVariants((current) => ({ ...current, [pointer]: index })),
      update: (pointer, value) => props.onChange(setAtPointer(props.value, pointer, value)),
      remove: (pointer) => props.onChange(removeAtPointer(props.value, pointer)),
    }),
    [props, variants],
  )

  if (rootType !== "object") {
    return (
      <div className="schema-root-error" role="alert">
        <AlertCircle />
        <span>插件配置 Schema 的根节点必须是 object。</span>
      </div>
    )
  }

  return (
    <FormContext.Provider value={context}>
      <div className="schema-form" style={{ "--schema-columns": rootUi.columns ?? 2 } as React.CSSProperties}>
        {rootErrors.length > 0 && (
          <div className="schema-root-validation" role="alert">
            <AlertCircle />
            <div>{rootErrors.map((error, index) => <span key={error.keyword + index}>{error.message}</span>)}</div>
          </div>
        )}
        <ObjectFields schema={rootSchema} pointer="" value={props.value} depth={0} ui={rootUi} />
      </div>
    </FormContext.Provider>
  )
}

function useFormContext() {
  const context = useContext(FormContext)
  if (!context) throw new Error("SchemaForm context is missing")
  return context
}

function ObjectFields({
  schema,
  pointer,
  value,
  depth,
  ui,
}: {
  schema: JsonSchema
  pointer: string
  value: JsonObject
  depth: number
  ui: UiOptions
}) {
  const context = useFormContext()
  const properties = asSchema(schema.properties)
  const required = new Set(
    Array.isArray(schema.required)
      ? schema.required.filter((item): item is string => typeof item === "string")
      : [],
  )
  const declared = new Set(Object.keys(properties))
  const additionalEntries = Object.keys(value).filter((name) => !declared.has(name))
  const allowAdditional = schema.additionalProperties === true
    || isPlainObject(schema.additionalProperties)
    || Object.keys(asSchema(schema.patternProperties)).length > 0

  return (
    <div
      className={cn("schema-object-fields", depth > 0 && "is-nested")}
      style={{ "--schema-columns": ui.columns ?? 2 } as React.CSSProperties}
    >
      <div className="schema-grid">
        {orderedProperties(schema, ui).map((name) => (
          <SchemaField
            key={name}
            rawSchema={asSchema(properties[name])}
            pointer={joinPointer(pointer, name)}
            name={name}
            required={required.has(name)}
            depth={depth + 1}
          />
        ))}
        {additionalEntries.map((name) => (
          <SchemaField
            key={name}
            rawSchema={propertySchema(schema, name)}
            pointer={joinPointer(pointer, name)}
            name={name}
            required={false}
            depth={depth + 1}
            dynamic
          />
        ))}
      </div>
      {allowAdditional && (
        <AdditionalPropertyControl
          schema={schema}
          pointer={pointer}
          value={value}
          disabled={context.disabled}
        />
      )}
    </div>
  )
}

function SchemaField({
  rawSchema,
  pointer,
  name,
  required,
  depth,
  dynamic = false,
}: {
  rawSchema: JsonSchema
  pointer: string
  name: string
  required: boolean
  depth: number
  dynamic?: boolean
}) {
  const context = useFormContext()
  const currentValue = getAtPointer(context.value, pointer)
  const rawVariants = schemaVariants(rawSchema)
  const defaultVariant = rawVariants.length
    ? matchingVariantIndex(rawVariants, currentValue, context.schema)
    : 0
  const variantIndex = context.variants[pointer] ?? defaultVariant
  const schema = materializeSchema(context.schema, rawSchema, currentValue, variantIndex)
  const ui = uiOptionsFor(context.uiSchema, pointer, schema)
  const type = schemaType(schema, currentValue)
  const controlId = useId()
  const exactErrors = errorsForPointer(context.errors, pointer)
  const nestedErrors = childErrorCount(context.errors, pointer)
  const title = ui.title ?? (typeof schema.title === "string" ? schema.title : humanizeProperty(name))
  const description = ui.description ?? (typeof schema.description === "string" ? schema.description : undefined)
  const readonly = schema.readOnly === true || ui.readonly === true
  const disabled = Boolean(context.disabled || ui.disabled || readonly)
  const wide = ui.width === "full" || ["object", "array", "unknown"].includes(type) || ui.widget === "textarea" || ui.widget === "code" || ui.widget === "json"

  if (ui.hidden) return null

  const removeField = () => {
    context.remove(pointer)
    const secrets = Object.fromEntries(
      Object.entries(context.secrets).filter(([secretPointer]) => secretPointer !== pointer && !secretPointer.startsWith(pointer + "/")),
    )
    context.onSecretsChange(secrets)
  }

  const activateCompound = () => {
    const empty = emptyValue(schema)
    const next = isJsonObject(empty) ? applySchemaDefaults(schema, empty) : empty
    context.update(pointer, next)
  }

  return (
    <section
      className={cn(
        "schema-field",
        `is-${type}`,
        wide && "is-wide",
        exactErrors.length > 0 && "has-error",
        readonly && "is-readonly",
      )}
      data-pointer={pointer}
    >
      <div className="schema-field-heading">
        <div className="min-w-0">
          <div className="schema-field-title-row">
            <label htmlFor={controlId}>{title}</label>
            {required ? <Badge variant="default">必填</Badge> : <Badge variant="neutral">可选</Badge>}
            {schema.deprecated === true && <Badge variant="warning">已弃用</Badge>}
            {readonly && <Badge variant="neutral">只读</Badge>}
            {nestedErrors > 0 && <Badge variant="danger">{nestedErrors} 处错误</Badge>}
          </div>
          <code>{name}</code>
          {description && <p>{description}</p>}
        </div>
        {!required && currentValue !== undefined && !readonly && (
          <ToolButton
            label={dynamic ? "删除自定义配置项" : "恢复为未设置"}
            variant="ghost"
            size="icon-sm"
            onClick={removeField}
            disabled={context.disabled}
          >
            {dynamic ? <Trash2 /> : <X />}
          </ToolButton>
        )}
      </div>

      {rawVariants.length > 0 && !enumOptions(rawSchema, ui) && (
        <VariantControl
          schema={rawSchema}
          pointer={pointer}
          currentValue={currentValue}
          selected={variantIndex}
          variants={rawVariants}
          disabled={disabled}
        />
      )}

      {currentValue === null && isNullableSchema(schema) ? (
        <div className="schema-unset-control">
          <Badge variant="neutral">null</Badge>
          <Button variant="outline" size="sm" onClick={activateCompound} disabled={disabled}>
            <Plus />
            设置值
          </Button>
        </div>
      ) : (type === "object" || type === "array") && currentValue === undefined ? (
        <button type="button" className="schema-add-compound" onClick={activateCompound} disabled={disabled}>
          <Plus />
          <span>{type === "object" ? "添加配置组" : "添加列表"}</span>
        </button>
      ) : (
        <FieldControl
          id={controlId}
          schema={schema}
          rawSchema={rawSchema}
          pointer={pointer}
          value={currentValue}
          required={required}
          disabled={disabled}
          depth={depth}
          ui={ui}
        />
      )}

      {ui.help && <div className="schema-field-help">{ui.help}</div>}
      {exactErrors.map((error, index) => (
        <div className="schema-field-error" role="alert" key={error.keyword + index}>
          <AlertCircle />
          <span>{error.message}</span>
        </div>
      ))}
    </section>
  )
}

function FieldControl({
  id,
  schema,
  rawSchema,
  pointer,
  value,
  required,
  disabled,
  depth,
  ui,
}: {
  id: string
  schema: JsonSchema
  rawSchema: JsonSchema
  pointer: string
  value: JsonValue | undefined
  required: boolean
  disabled: boolean
  depth: number
  ui: UiOptions
}) {
  const type = schemaType(schema, value)
  if (schema.readOnly === true || ui.readonly) return <ReadonlyControl value={value} />
  const options = enumOptions(schema, ui)
  if (options && type !== "array") return <EnumControl id={id} pointer={pointer} value={value} options={options} disabled={disabled} ui={ui} />
  if (type === "object") {
    const objectValue = isJsonObject(value) ? value : {}
    return <ObjectFields schema={schema} pointer={pointer} value={objectValue} depth={depth} ui={ui} />
  }
  if (type === "array") {
    return <ArrayControl schema={schema} rawSchema={rawSchema} pointer={pointer} value={Array.isArray(value) ? value : []} disabled={disabled} ui={ui} depth={depth} />
  }
  if (type === "boolean") return <BooleanControl id={id} pointer={pointer} value={value} required={required} disabled={disabled} />
  if (type === "integer" || type === "number") return <NumberControl id={id} schema={schema} pointer={pointer} value={value} required={required} disabled={disabled} integer={type === "integer"} ui={ui} />
  if (type === "string") return <StringControl id={id} schema={schema} pointer={pointer} value={value} disabled={disabled} ui={ui} />
  return <JsonControl id={id} pointer={pointer} value={value} disabled={disabled} rows={ui.rows} />
}

function StringControl({
  id,
  schema,
  pointer,
  value,
  disabled,
  ui,
}: {
  id: string
  schema: JsonSchema
  pointer: string
  value: JsonValue | undefined
  disabled: boolean
  ui: UiOptions
}) {
  const context = useFormContext()
  const options = enumOptions(schema, ui)
  if (isSecretSchema(schema) || ui.widget === "password") {
    return <SecretControl id={id} pointer={pointer} disabled={disabled} placeholder={ui.placeholder} />
  }
  if (options) return <EnumControl id={id} pointer={pointer} value={value} options={options} disabled={disabled} ui={ui} />

  const stringValue = typeof value === "string" ? value : ""
  const format = typeof schema.format === "string" ? schema.format : ""
  const multiline = ["textarea", "code"].includes(ui.widget ?? "") || ["multiline", "code"].includes(format)
  const common = {
    id,
    value: stringValue,
    disabled,
    placeholder: ui.placeholder,
    minLength: numberKeyword(schema.minLength),
    maxLength: numberKeyword(schema.maxLength),
    onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => context.update(pointer, event.target.value),
  }
  if (multiline) {
    return (
      <Textarea
        {...common}
        rows={ui.rows ?? (ui.widget === "code" || format === "code" ? 8 : 4)}
        className={cn((ui.widget === "code" || format === "code") && "font-mono text-xs")}
      />
    )
  }
  if (format === "color" || ui.widget === "color") {
    return (
      <div className="schema-color-control">
        <input
          type="color"
          value={/^#[0-9a-f]{6}$/i.test(stringValue) ? stringValue : "#000000"}
          disabled={disabled}
          aria-label="选择颜色"
          onChange={(event) => context.update(pointer, event.target.value)}
        />
        <Input {...common} placeholder={ui.placeholder ?? "#000000"} />
      </div>
    )
  }
  const inputType = format === "email" ? "email" : format === "uri" || format === "url" ? "url" : format === "date" ? "date" : "text"
  return (
    <div className={cn("schema-input-with-unit", !ui.unit && "has-no-unit") }>
      <Input {...common} type={inputType} pattern={typeof schema.pattern === "string" ? schema.pattern : undefined} />
      {ui.unit && <span>{ui.unit}</span>}
    </div>
  )
}

function SecretControl({
  id,
  pointer,
  disabled,
  placeholder,
}: {
  id: string
  pointer: string
  disabled: boolean
  placeholder?: string
}) {
  const context = useFormContext()
  const [revealed, setRevealed] = useState(false)
  const entry = context.secrets[pointer] ?? {}
  const pending = entry.update !== undefined
  const inputValue = typeof entry.update === "string" ? entry.update : ""
  const configured = Boolean(entry.source)

  const updateEntry = (nextEntry: typeof entry) => {
    const next = { ...context.secrets }
    if (!nextEntry.source && nextEntry.update === undefined) delete next[pointer]
    else next[pointer] = nextEntry
    context.onSecretsChange(next)
  }

  const status = entry.update === null
    ? { label: "待清除", variant: "warning" as const }
    : typeof entry.update === "string"
      ? { label: configured ? "待替换" : "待设置", variant: "default" as const }
      : configured
        ? { label: "已配置", variant: "success" as const }
        : { label: "未配置", variant: "neutral" as const }

  return (
    <div className="schema-secret-control">
      <div className="schema-secret-input">
        <KeyRound />
        <Input
          id={id}
          type={revealed ? "text" : "password"}
          value={inputValue}
          disabled={disabled}
          autoComplete="new-password"
          placeholder={entry.update === null ? "保存后清除" : placeholder ?? (configured ? "输入新值以替换" : "输入密钥")}
          onChange={(event) => {
            const nextValue = event.target.value
            updateEntry(nextValue ? { ...entry, update: nextValue } : { source: entry.source })
          }}
        />
        <ToolButton
          label={revealed ? "隐藏待提交密钥" : "显示待提交密钥"}
          variant="ghost"
          size="icon-sm"
          onClick={() => setRevealed((current) => !current)}
          disabled={disabled || !inputValue}
        >
          {revealed ? <EyeOff /> : <Eye />}
        </ToolButton>
      </div>
      <div className="schema-secret-actions">
        <Badge variant={status.variant}>{status.label}</Badge>
        {pending && (
          <Button variant="ghost" size="sm" onClick={() => updateEntry({ source: entry.source })} disabled={disabled}>
            <RotateCcw />
            撤销
          </Button>
        )}
        {(configured || typeof entry.update === "string") && entry.update !== null && (
          <Button variant="ghost" size="sm" onClick={() => updateEntry({ ...entry, update: null })} disabled={disabled}>
            <Trash2 />
            清除
          </Button>
        )}
      </div>
    </div>
  )
}

function EnumControl({
  id,
  pointer,
  value,
  options,
  disabled,
  ui,
}: {
  id: string
  pointer: string
  value: JsonValue | undefined
  options: NonNullable<ReturnType<typeof enumOptions>>
  disabled: boolean
  ui: UiOptions
}) {
  const context = useFormContext()
  const widget = ui.widget ?? (options.length <= 6 ? "badges" : "select")
  if (widget === "select" || widget === "dropdown") {
    const selected = options.findIndex((option) => enumKey(option.value) === enumKey(value ?? null))
    return (
      <Select
        id={id}
        value={selected >= 0 ? String(selected) : ""}
        disabled={disabled}
        onChange={(event) => {
          const option = options[Number(event.target.value)]
          if (option) context.update(pointer, cloneJson(option.value))
        }}
      >
        {selected < 0 && <option value="">未设置</option>}
        {options.map((option, index) => <option value={index} key={enumKey(option.value)}>{option.label}</option>)}
      </Select>
    )
  }
  return (
    <div id={id} className={cn("schema-enum-options", widget === "radio" && "is-radio")} role="radiogroup">
      {options.map((option) => {
        const selected = enumKey(option.value) === enumKey(value ?? null)
        return (
          <button
            type="button"
            role="radio"
            aria-checked={selected}
            className={selected ? "is-selected" : ""}
            key={enumKey(option.value)}
            disabled={disabled}
            onClick={() => context.update(pointer, cloneJson(option.value))}
          >
            <span className="schema-option-mark">{selected && <Check />}</span>
            <span><strong>{option.label}</strong>{option.description && <small>{option.description}</small>}</span>
          </button>
        )
      })}
    </div>
  )
}

function BooleanControl({
  id,
  pointer,
  value,
  required,
  disabled,
}: {
  id: string
  pointer: string
  value: JsonValue | undefined
  required: boolean
  disabled: boolean
}) {
  const context = useFormContext()
  if (!required) {
    return (
      <div id={id} className="schema-boolean-segment" role="group" aria-label="布尔值">
        {[
          { label: "未设置", value: undefined },
          { label: "开启", value: true },
          { label: "关闭", value: false },
        ].map((option) => {
          const selected = value === option.value
          return (
            <button
              type="button"
              key={option.label}
              className={selected ? "is-selected" : ""}
              disabled={disabled}
              onClick={() => option.value === undefined ? context.remove(pointer) : context.update(pointer, option.value)}
            >
              {selected && <Check />}
              {option.label}
            </button>
          )
        })}
      </div>
    )
  }
  const checked = value === true
  return (
    <div className="schema-switch-control">
      <Switch id={id} checked={checked} disabled={disabled} onCheckedChange={(next) => context.update(pointer, next)} />
      <Badge variant={checked ? "success" : "neutral"}>{checked ? "已开启" : "已关闭"}</Badge>
    </div>
  )
}

function NumberControl({
  id,
  schema,
  pointer,
  value,
  required,
  disabled,
  integer,
  ui,
}: {
  id: string
  schema: JsonSchema
  pointer: string
  value: JsonValue | undefined
  required: boolean
  disabled: boolean
  integer: boolean
  ui: UiOptions
}) {
  const context = useFormContext()
  const minimum = numberKeyword(schema.minimum)
  const maximum = numberKeyword(schema.maximum)
  const step = ui.step ?? numberKeyword(schema.multipleOf) ?? (integer ? 1 : "any")
  const numericValue = typeof value === "number" ? value : undefined
  const clamp = (next: number) => Math.min(maximum ?? Number.POSITIVE_INFINITY, Math.max(minimum ?? Number.NEGATIVE_INFINITY, next))
  const increment = typeof step === "number" ? step : integer ? 1 : 0.1
  const setNumber = (next: number) => context.update(pointer, integer ? Math.round(clamp(next)) : clamp(next))
  const slider = (ui.widget === "slider" || ui.widget === "range") && minimum !== undefined && maximum !== undefined

  return (
    <div className={cn("schema-number-control", slider && "has-slider") }>
      {slider && (
        <input
          type="range"
          min={minimum}
          max={maximum}
          step={typeof step === "number" ? step : undefined}
          value={numericValue ?? minimum}
          disabled={disabled}
          aria-label="调整数值"
          onChange={(event) => setNumber(Number(event.target.value))}
        />
      )}
      <div className="schema-number-input">
        <ToolButton label="减小" variant="outline" size="icon-sm" disabled={disabled || numericValue === undefined} onClick={() => setNumber((numericValue ?? 0) - increment)}>
          <Minus />
        </ToolButton>
        <Input
          id={id}
          type="number"
          value={numericValue ?? ""}
          disabled={disabled}
          required={required}
          min={minimum}
          max={maximum}
          step={step}
          placeholder={ui.placeholder}
          onChange={(event) => {
            if (!event.target.value) {
              if (!required) context.remove(pointer)
              return
            }
            setNumber(Number(event.target.value))
          }}
        />
        {ui.unit && <span>{ui.unit}</span>}
        <ToolButton label="增大" variant="outline" size="icon-sm" disabled={disabled} onClick={() => setNumber((numericValue ?? minimum ?? 0) + increment)}>
          <Plus />
        </ToolButton>
      </div>
    </div>
  )
}

function ArrayControl({
  schema,
  rawSchema,
  pointer,
  value,
  disabled,
  ui,
  depth,
}: {
  schema: JsonSchema
  rawSchema: JsonSchema
  pointer: string
  value: JsonValue[]
  disabled: boolean
  ui: UiOptions
  depth: number
}) {
  const context = useFormContext()
  const itemOptions = enumOptions(asSchema(schema.items), ui)
  if (itemOptions && schema.uniqueItems === true) {
    return <MultiEnumControl pointer={pointer} value={value} options={itemOptions} disabled={disabled} schema={schema} />
  }

  const minItems = numberKeyword(schema.minItems) ?? 0
  const maxItems = numberKeyword(schema.maxItems)
  const tuple = Array.isArray(schema.prefixItems)
  const canAdd = !disabled && (maxItems === undefined || value.length < maxItems) && schema.items !== false

  const updateArray = (next: JsonValue[]) => context.update(pointer, next)
  const removeItem = (index: number) => {
    if (value.length <= minItems) return
    updateArray(value.filter((_, itemIndex) => itemIndex !== index))
    context.onSecretsChange(remapSecretsAfterArrayOperation(context.secrets, pointer, { type: "remove", index }))
  }
  const moveItem = (from: number, to: number) => {
    if (tuple || to < 0 || to >= value.length) return
    const next = [...value]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    updateArray(next)
    context.onSecretsChange(remapSecretsAfterArrayOperation(context.secrets, pointer, { type: "move", from, to }))
  }
  const addItem = (copy?: JsonValue) => {
    if (!canAdd) return
    const item = copy === undefined ? defaultArrayItem(context.schema, rawSchema, value.length) : cloneJson(copy)
    updateArray([...value, item])
  }

  return (
    <div className="schema-array-control">
      <div className="schema-array-toolbar">
        <span><strong>{value.length}</strong> 项</span>
        <Button variant="outline" size="sm" onClick={() => addItem()} disabled={!canAdd}>
          <Plus />
          {ui.addLabel ?? "添加一项"}
        </Button>
      </div>
      <div className="schema-array-items">
        {value.map((item, index) => {
          const itemSchema = materializeSchema(context.schema, arrayItemSchema(schema, index), item)
          const itemPointer = joinPointer(pointer, index)
          const itemTitle = arrayItemTitle(item, index, ui)
          return (
            <ArrayItem
              key={index}
              title={itemTitle}
              errorCount={childErrorCount(context.errors, itemPointer) + errorsForPointer(context.errors, itemPointer).length}
              collapsible={ui.collapsible !== false && ["object", "array"].includes(schemaType(itemSchema, item))}
              collapsed={ui.collapsed === true}
              actions={
                <>
                  {!tuple && <ToolButton label="上移" variant="ghost" size="icon-sm" disabled={disabled || index === 0} onClick={() => moveItem(index, index - 1)}><ChevronUp /></ToolButton>}
                  {!tuple && <ToolButton label="下移" variant="ghost" size="icon-sm" disabled={disabled || index === value.length - 1} onClick={() => moveItem(index, index + 1)}><ChevronDown /></ToolButton>}
                  <ToolButton label="复制到末尾" variant="ghost" size="icon-sm" disabled={!canAdd} onClick={() => addItem(item)}><Copy /></ToolButton>
                  <ToolButton label="删除" variant="ghost" size="icon-sm" disabled={disabled || value.length <= minItems} onClick={() => removeItem(index)}><Trash2 /></ToolButton>
                </>
              }
            >
              <ArrayItemControl
                schema={itemSchema}
                rawSchema={arrayItemSchema(rawSchema, index)}
                pointer={itemPointer}
                value={item}
                disabled={disabled}
                depth={depth + 1}
                ui={uiOptionsFor(context.uiSchema, itemPointer, itemSchema)}
              />
            </ArrayItem>
          )
        })}
        {value.length === 0 && <div className="schema-array-empty">{ui.emptyLabel ?? "暂无配置项"}</div>}
      </div>
    </div>
  )
}

function ArrayItem({
  title,
  errorCount,
  collapsible,
  collapsed,
  actions,
  children,
}: {
  title: string
  errorCount: number
  collapsible: boolean
  collapsed: boolean
  actions: React.ReactNode
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(!collapsed)
  return (
    <div className={cn("schema-array-item", !open && "is-collapsed") }>
      <div className="schema-array-item-head">
        {collapsible ? (
          <button type="button" className="schema-array-item-toggle" onClick={() => setOpen((current) => !current)} aria-expanded={open}>
            <ChevronDown />
            <strong>{title}</strong>
          </button>
        ) : <strong>{title}</strong>}
        {errorCount > 0 && <Badge variant="danger">{errorCount} 处错误</Badge>}
        <div className="schema-array-item-actions">{actions}</div>
      </div>
      {open && <div className="schema-array-item-body">{children}</div>}
    </div>
  )
}

function ArrayItemControl({
  schema,
  rawSchema,
  pointer,
  value,
  disabled,
  depth,
  ui,
}: {
  schema: JsonSchema
  rawSchema: JsonSchema
  pointer: string
  value: JsonValue
  disabled: boolean
  depth: number
  ui: UiOptions
}) {
  const type = schemaType(schema, value)
  if (type === "object" && isJsonObject(value)) return <ObjectFields schema={schema} pointer={pointer} value={value} depth={depth} ui={ui} />
  return <FieldControl id={pointer} schema={schema} rawSchema={rawSchema} pointer={pointer} value={value} required disabled={disabled} depth={depth} ui={ui} />
}

function MultiEnumControl({
  pointer,
  value,
  options,
  disabled,
  schema,
}: {
  pointer: string
  value: JsonValue[]
  options: NonNullable<ReturnType<typeof enumOptions>>
  disabled: boolean
  schema: JsonSchema
}) {
  const context = useFormContext()
  const minItems = numberKeyword(schema.minItems) ?? 0
  const maxItems = numberKeyword(schema.maxItems)
  return (
    <div className="schema-multi-options" role="group">
      {options.map((option) => {
        const key = enumKey(option.value)
        const selected = value.some((item) => enumKey(item) === key)
        const blocked = disabled || (!selected && maxItems !== undefined && value.length >= maxItems) || (selected && value.length <= minItems)
        return (
          <button
            type="button"
            className={selected ? "is-selected" : ""}
            aria-pressed={selected}
            disabled={blocked}
            key={key}
            onClick={() => context.update(pointer, selected ? value.filter((item) => enumKey(item) !== key) : [...value, cloneJson(option.value)])}
          >
            <span>{selected && <Check />}</span>
            {option.label}
          </button>
        )
      })}
    </div>
  )
}

function VariantControl({
  schema,
  pointer,
  currentValue,
  selected,
  variants,
  disabled,
}: {
  schema: JsonSchema
  pointer: string
  currentValue: JsonValue | undefined
  selected: number
  variants: JsonSchema[]
  disabled: boolean
}) {
  const context = useFormContext()
  const choose = (index: number) => {
    context.setVariant(pointer, index)
    const selectedSchema = materializeSchema(context.schema, schema, currentValue, index)
    if (isJsonObject(currentValue) && schemaType(selectedSchema, currentValue) === "object") {
      const selectedProperties = new Set(Object.keys(asSchema(selectedSchema.properties)))
      const variantProperties = new Set(variants.flatMap((variant) => Object.keys(asSchema(materializeSchema(context.schema, variant, currentValue).properties))))
      const next = cloneJson(currentValue)
      for (const property of variantProperties) {
        if (!selectedProperties.has(property)) delete next[property]
      }
      for (const [property, child] of Object.entries(asSchema(selectedSchema.properties))) {
        const childSchema = asSchema(child)
        if ("const" in childSchema && isJsonValue(childSchema.const)) {
          next[property] = cloneJson(childSchema.const)
        }
      }
      context.update(pointer, applySchemaDefaults(selectedSchema, next))
    } else {
      context.update(pointer, emptyValue(selectedSchema))
    }
  }
  return (
    <div className="schema-variant-control">
      <span>配置形式</span>
      <div role="tablist">
        {variants.map((variant, index) => (
          <button type="button" role="tab" aria-selected={selected === index} className={selected === index ? "is-selected" : ""} disabled={disabled} key={index} onClick={() => choose(index)}>
            {selected === index && <Check />}
            {typeof variant.title === "string" ? variant.title : `选项 ${index + 1}`}
          </button>
        ))}
      </div>
    </div>
  )
}

function AdditionalPropertyControl({
  schema,
  pointer,
  value,
  disabled,
}: {
  schema: JsonSchema
  pointer: string
  value: JsonObject
  disabled?: boolean
}) {
  const context = useFormContext()
  const [draft, setDraft] = useState("")
  const [error, setError] = useState("")
  const add = () => {
    const name = draft.trim()
    if (!name) return
    if (name in value || name in asSchema(schema.properties)) {
      setError("配置键已存在")
      return
    }
    const childSchema = propertySchema(schema, name)
    context.update(joinPointer(pointer, name), emptyValue(childSchema))
    setDraft("")
    setError("")
  }
  return (
    <div className="schema-additional-property">
      <Input
        value={draft}
        disabled={disabled}
        aria-label="新增自定义配置键"
        placeholder="自定义配置键"
        onChange={(event) => { setDraft(event.target.value); setError("") }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault()
            add()
          }
        }}
      />
      <Button variant="outline" size="sm" onClick={add} disabled={disabled || !draft.trim()}>
        <Plus />
        添加字段
      </Button>
      {error && <span role="alert">{error}</span>}
    </div>
  )
}

function JsonControl({
  id,
  pointer,
  value,
  disabled,
  rows,
}: {
  id: string
  pointer: string
  value: JsonValue | undefined
  disabled: boolean
  rows?: number
}) {
  const context = useFormContext()
  const [text, setText] = useState(value === undefined ? "" : JSON.stringify(value, null, 2))
  const [error, setError] = useState("")
  useEffect(() => setText(value === undefined ? "" : JSON.stringify(value, null, 2)), [value])
  const commit = () => {
    if (!text.trim()) {
      context.remove(pointer)
      setError("")
      return
    }
    try {
      context.update(pointer, JSON.parse(text) as JsonValue)
      setError("")
    } catch {
      setError("JSON 格式不正确")
    }
  }
  return (
    <div className="schema-json-control">
      <Textarea id={id} className="font-mono text-xs" rows={rows ?? 8} value={text} disabled={disabled} onChange={(event) => setText(event.target.value)} onBlur={commit} />
      {error && <span role="alert"><AlertCircle />{error}</span>}
    </div>
  )
}

function ReadonlyControl({ value }: { value: JsonValue | undefined }) {
  return <pre className="schema-readonly-value">{value === undefined ? "未设置" : typeof value === "string" ? value : JSON.stringify(value, null, 2)}</pre>
}

function ToolButton({ label, children, ...props }: ButtonProps & { label: string; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button type="button" aria-label={label} {...props}>{children}</Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

function arrayItemTitle(item: JsonValue, index: number, ui: UiOptions) {
  if (ui.itemTitle && isJsonObject(item)) {
    const candidate = item[ui.itemTitle]
    if (typeof candidate === "string" || typeof candidate === "number") return String(candidate)
  }
  return `${ui.itemLabel ?? "项目"} ${index + 1}`
}

function isJsonValue(value: unknown): value is JsonValue {
  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") return true
  if (Array.isArray(value)) return value.every(isJsonValue)
  return isPlainObject(value) && Object.values(value).every(isJsonValue)
}
