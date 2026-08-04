import {
  registerSchema,
  unregisterSchema,
  validate as compileSchema,
  type OutputUnit,
  type Validator as SchemaValidator,
} from "@hyperjump/json-schema/draft-2020-12"

import type { JsonValue } from "@/lib/api"

export type JsonObject = Record<string, JsonValue>
export type JsonSchema = Record<string, unknown>

export interface SecretDraftEntry {
  source?: string
  update?: string | null
}

export type SecretDraft = Record<string, SecretDraftEntry>

export interface FormValidationError {
  pointer: string
  message: string
  keyword: string
}

export interface UiOptions {
  widget?: string
  title?: string
  description?: string
  placeholder?: string
  help?: string
  unit?: string
  order?: string[]
  hidden?: boolean
  disabled?: boolean
  readonly?: boolean
  rows?: number
  step?: number
  columns?: number
  width?: "full" | "half" | string
  addLabel?: string
  itemLabel?: string
  itemTitle?: string
  emptyLabel?: string
  labels?: Record<string, string>
  collapsible?: boolean
  collapsed?: boolean
}

export function asSchema(value: unknown): JsonSchema {
  return isPlainObject(value) ? value : {}
}

export function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

export function isJsonObject(value: JsonValue | undefined): value is JsonObject {
  return isPlainObject(value)
}

export function cloneJson<T extends JsonValue>(value: T): T {
  return structuredClone(value)
}

export function encodePointerSegment(segment: string) {
  return segment.replaceAll("~", "~0").replaceAll("/", "~1")
}

export function decodePointerSegment(segment: string) {
  return segment.replaceAll("~1", "/").replaceAll("~0", "~")
}

export function joinPointer(parent: string, segment: string | number) {
  return parent + "/" + encodePointerSegment(String(segment))
}

export function pointerSegments(pointer: string) {
  if (!pointer) return []
  return pointer
    .slice(1)
    .split("/")
    .map(decodePointerSegment)
}

export function getAtPointer(root: JsonValue, pointer: string): JsonValue | undefined {
  let current: JsonValue | undefined = root
  for (const segment of pointerSegments(pointer)) {
    if (Array.isArray(current)) {
      const index = Number(segment)
      current = Number.isInteger(index) ? current[index] : undefined
    } else if (isJsonObject(current)) {
      current = current[segment]
    } else {
      return undefined
    }
  }
  return current
}

export function setAtPointer(root: JsonObject, pointer: string, value: JsonValue): JsonObject {
  if (!pointer) return isJsonObject(value) ? cloneJson(value) : root
  const next = cloneJson(root)
  const segments = pointerSegments(pointer)
  let current: JsonValue = next
  for (let index = 0; index < segments.length - 1; index += 1) {
    const segment = segments[index]
    const following = segments[index + 1]
    if (Array.isArray(current)) {
      const itemIndex = Number(segment)
      if (!Number.isInteger(itemIndex) || itemIndex < 0 || itemIndex >= current.length) return next
      const child = current[itemIndex]
      if (!isJsonObject(child) && !Array.isArray(child)) {
        current[itemIndex] = /^\d+$/.test(following) ? [] : {}
      }
      current = current[itemIndex]
    } else if (isJsonObject(current)) {
      const child = current[segment]
      if (!isJsonObject(child) && !Array.isArray(child)) {
        current[segment] = /^\d+$/.test(following) ? [] : {}
      }
      current = current[segment]
    } else {
      return next
    }
  }
  const last = segments.at(-1)
  if (last === undefined) return next
  if (Array.isArray(current)) {
    const index = Number(last)
    if (Number.isInteger(index) && index >= 0 && index < current.length) current[index] = cloneJson(value)
  } else if (isJsonObject(current)) {
    current[last] = cloneJson(value)
  }
  return next
}

export function removeAtPointer(root: JsonObject, pointer: string): JsonObject {
  if (!pointer) return {}
  const next = cloneJson(root)
  const segments = pointerSegments(pointer)
  let current: JsonValue = next
  for (const segment of segments.slice(0, -1)) {
    if (Array.isArray(current)) {
      current = current[Number(segment)]
    } else if (isJsonObject(current)) {
      current = current[segment]
    } else {
      return next
    }
    if (current === undefined) return next
  }
  const last = segments.at(-1)
  if (last === undefined) return next
  if (Array.isArray(current)) {
    const index = Number(last)
    if (Number.isInteger(index) && index >= 0 && index < current.length) current.splice(index, 1)
  } else if (isJsonObject(current)) {
    delete current[last]
  }
  return next
}

export function materializeSchema(
  root: JsonSchema,
  schema: JsonSchema,
  value: JsonValue | undefined,
  variantIndex?: number,
): JsonSchema {
  let result = resolveSchema(root, schema, new Set(), 0)

  const allOf = Array.isArray(result.allOf) ? result.allOf.map(asSchema) : []
  if (allOf.length > 0) {
    delete result.allOf
    for (const branch of allOf) {
      result = mergeSchemas(result, materializeSchema(root, branch, value))
    }
  }

  const conditional = asSchema(result.if)
  if (Object.keys(conditional).length > 0) {
    const base = { ...result }
    delete base.if
    delete base.then
    delete base.else
    result = mergeSchemas(
      base,
      schemaMatches(root, conditional, value) ? asSchema(result.then) : asSchema(result.else),
    )
  }

  const dependentSchemas = asSchema(result.dependentSchemas)
  if (isPlainObject(value)) {
    for (const [property, dependent] of Object.entries(dependentSchemas)) {
      if (property in value) result = mergeSchemas(result, asSchema(dependent))
    }
  }

  const variants = schemaVariants(result)
  if (variants.length > 0 && !enumOptions(result)) {
    const selected = variantIndex ?? matchingVariantIndex(variants, value, root)
    result = mergeSchemas(withoutComposition(result), resolveSchema(root, variants[selected] ?? variants[0], new Set(), 0))
  }
  return result
}

function resolveSchema(
  root: JsonSchema,
  schema: JsonSchema,
  seenReferences: Set<string>,
  depth: number,
): JsonSchema {
  if (depth > 64) return schema
  let result = { ...schema }
  const reference = typeof result.$ref === "string" ? result.$ref : undefined
  if (reference?.startsWith("#") && !seenReferences.has(reference)) {
    const target = resolveLocalReference(root, reference)
    if (target) {
      const nextSeen = new Set(seenReferences)
      nextSeen.add(reference)
      const siblings = { ...result }
      delete siblings.$ref
      result = mergeSchemas(resolveSchema(root, target, nextSeen, depth + 1), siblings)
    }
  }
  return result
}

function resolveLocalReference(root: JsonSchema, reference: string): JsonSchema | undefined {
  if (reference === "#") return root
  let current: unknown = root
  for (const segment of pointerSegments(reference.slice(1))) {
    if (!isPlainObject(current) && !Array.isArray(current)) return undefined
    current = Array.isArray(current) ? current[Number(segment)] : current[segment]
  }
  return isPlainObject(current) ? current : undefined
}

export function mergeSchemas(left: JsonSchema, right: JsonSchema): JsonSchema {
  const result: JsonSchema = { ...left, ...right }
  for (const key of ["properties", "patternProperties", "$defs", "dependentSchemas"]) {
    const leftObject = asSchema(left[key])
    const rightObject = asSchema(right[key])
    if (Object.keys(leftObject).length > 0 || Object.keys(rightObject).length > 0) {
      result[key] = { ...leftObject, ...rightObject }
    }
  }
  const required = [
    ...(Array.isArray(left.required) ? left.required.filter((item): item is string => typeof item === "string") : []),
    ...(Array.isArray(right.required) ? right.required.filter((item): item is string => typeof item === "string") : []),
  ]
  if (required.length > 0) result.required = [...new Set(required)]
  return result
}

function withoutComposition(schema: JsonSchema): JsonSchema {
  const result = { ...schema }
  delete result.oneOf
  delete result.anyOf
  return result
}

export function schemaVariants(schema: JsonSchema): JsonSchema[] {
  const variants = Array.isArray(schema.oneOf)
    ? schema.oneOf
    : Array.isArray(schema.anyOf)
      ? schema.anyOf
      : []
  return variants.map(asSchema)
}

export function matchingVariantIndex(
  variants: JsonSchema[],
  value: JsonValue | undefined,
  root: JsonSchema = {},
) {
  const index = variants.findIndex((variant) => schemaMatches(root, variant, value))
  return index >= 0 ? index : 0
}

function schemaMatches(root: JsonSchema, schema: JsonSchema, value: JsonValue | undefined): boolean {
  const resolved = resolveSchema(root, schema, new Set(), 0)
  if ("const" in resolved && !jsonEquals(resolved.const, value)) return false
  if (Array.isArray(resolved.enum) && !resolved.enum.some((candidate) => jsonEquals(candidate, value))) return false

  const declared = Array.isArray(resolved.type)
    ? resolved.type.filter((item): item is string => typeof item === "string")
    : typeof resolved.type === "string"
      ? [resolved.type]
      : []
  if (declared.length > 0 && !declared.some((type) => valueMatchesType(type, value))) return false

  if (isJsonObject(value)) {
    const required = Array.isArray(resolved.required)
      ? resolved.required.filter((item): item is string => typeof item === "string")
      : []
    if (required.some((name) => !(name in value))) return false
    for (const [name, child] of Object.entries(asSchema(resolved.properties))) {
      if (name in value && !schemaMatches(root, asSchema(child), value[name])) return false
    }
  }

  const allOf = Array.isArray(resolved.allOf) ? resolved.allOf.map(asSchema) : []
  if (allOf.some((branch) => !schemaMatches(root, branch, value))) return false
  const anyOf = Array.isArray(resolved.anyOf) ? resolved.anyOf.map(asSchema) : []
  if (anyOf.length > 0 && !anyOf.some((branch) => schemaMatches(root, branch, value))) return false
  const oneOf = Array.isArray(resolved.oneOf) ? resolved.oneOf.map(asSchema) : []
  if (oneOf.length > 0 && oneOf.filter((branch) => schemaMatches(root, branch, value)).length !== 1) return false
  if (resolved.not && schemaMatches(root, asSchema(resolved.not), value)) return false
  return true
}

function valueMatchesType(type: string, value: JsonValue | undefined) {
  if (type === "null") return value === null
  if (value === undefined) return false
  if (type === "object") return isJsonObject(value)
  if (type === "array") return Array.isArray(value)
  if (type === "boolean") return typeof value === "boolean"
  if (type === "integer") return typeof value === "number" && Number.isInteger(value)
  if (type === "number") return typeof value === "number" && Number.isFinite(value)
  if (type === "string") return typeof value === "string"
  return true
}

function jsonEquals(left: unknown, right: unknown) {
  return JSON.stringify(left) === JSON.stringify(right)
}

export interface EnumOption {
  value: JsonValue
  label: string
  description?: string
}

export function enumOptions(schema: JsonSchema, ui?: UiOptions): EnumOption[] | undefined {
  let values: JsonValue[] | undefined
  let titles: Array<string | undefined> = []
  let descriptions: Array<string | undefined> = []
  if (Array.isArray(schema.enum)) {
    values = schema.enum.filter(isJsonValue)
    const names = Array.isArray(schema["x-enumNames"])
      ? schema["x-enumNames"]
      : Array.isArray(schema.enumNames)
        ? schema.enumNames
        : []
    titles = names.map((item) => (typeof item === "string" ? item : undefined))
  } else {
    const variants = schemaVariants(schema)
    if (variants.length === 0 || variants.some((variant) => !("const" in variant))) return undefined
    values = variants.map((variant) => variant.const).filter(isJsonValue)
    titles = variants.map((variant) => (typeof variant.title === "string" ? variant.title : undefined))
    descriptions = variants.map((variant) => (typeof variant.description === "string" ? variant.description : undefined))
  }
  return values.map((value, index) => {
    const key = enumKey(value)
    return {
      value,
      label: ui?.labels?.[key] ?? titles[index] ?? displayJsonValue(value),
      description: descriptions[index],
    }
  })
}

export function enumKey(value: JsonValue) {
  return typeof value === "string" ? value : JSON.stringify(value)
}

export function schemaType(schema: JsonSchema, value?: JsonValue): string {
  const declared = Array.isArray(schema.type)
    ? schema.type.filter((item): item is string => typeof item === "string" && item !== "null")
    : typeof schema.type === "string"
      ? [schema.type]
      : []
  if (value !== undefined && value !== null) {
    if (Array.isArray(value) && declared.includes("array")) return "array"
    if (isJsonObject(value) && declared.includes("object")) return "object"
    if (typeof value === "boolean" && declared.includes("boolean")) return "boolean"
    if (typeof value === "number" && declared.includes("integer") && Number.isInteger(value)) return "integer"
    if (typeof value === "number" && declared.includes("number")) return "number"
    if (typeof value === "string" && declared.includes("string")) return "string"
  }
  if (declared.length > 0) return declared[0]
  if (isPlainObject(schema.properties) || "additionalProperties" in schema || isPlainObject(schema.patternProperties)) return "object"
  if ("items" in schema || Array.isArray(schema.prefixItems)) return "array"
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    const first = schema.enum[0]
    if (Array.isArray(first)) return "array"
    if (isPlainObject(first)) return "object"
    return typeof first
  }
  if ("const" in schema && isJsonValue(schema.const)) return typeof schema.const
  return "unknown"
}

export function isNullableSchema(schema: JsonSchema) {
  return schema.type === "null" || (Array.isArray(schema.type) && schema.type.includes("null"))
}

export function isSecretSchema(schema: JsonSchema) {
  return schema.writeOnly === true || schema["x-qimen-secret"] === true || schema.format === "password"
}

export function applySchemaDefaults(schema: JsonSchema, value: JsonObject): JsonObject {
  const hydrated = hydrateValue(schema, schema, value, true)
  return isJsonObject(hydrated) ? hydrated : value
}

function hydrateValue(
  root: JsonSchema,
  rawSchema: JsonSchema,
  value: JsonValue | undefined,
  required: boolean,
): JsonValue | undefined {
  const schema = materializeSchema(root, rawSchema, value)
  let next = value === undefined ? explicitDefault(schema) : cloneJson(value)
  if (next === undefined && required) next = emptyValue(schema)
  if (next === undefined) return undefined

  const type = schemaType(schema, next)
  if (type === "object" && isJsonObject(next)) {
    const properties = asSchema(schema.properties)
    const requiredProperties = new Set(
      Array.isArray(schema.required)
        ? schema.required.filter((item): item is string => typeof item === "string")
        : [],
    )
    for (const [name, child] of Object.entries(properties)) {
      const childSchema = asSchema(child)
      if (isSecretSchema(childSchema) && next[name] === undefined) continue
      const childValue = hydrateValue(root, childSchema, next[name], requiredProperties.has(name))
      if (childValue !== undefined) next[name] = childValue
    }
  } else if (type === "array" && Array.isArray(next)) {
    const minimum = numberKeyword(schema.minItems) ?? 0
    while (next.length < minimum) next.push(defaultArrayItem(root, schema, next.length))
    for (let index = 0; index < next.length; index += 1) {
      const itemSchema = arrayItemSchema(schema, index)
      next[index] = hydrateValue(root, itemSchema, next[index], true) ?? null
    }
  }
  return next
}

function explicitDefault(schema: JsonSchema): JsonValue | undefined {
  if (isSecretSchema(schema)) return undefined
  if (isJsonValue(schema.default)) return cloneJson(schema.default)
  if (isJsonValue(schema.const)) return cloneJson(schema.const)
  return undefined
}

export function emptyValue(schema: JsonSchema): JsonValue {
  const direct = explicitDefault(schema)
  if (direct !== undefined) return direct
  const options = enumOptions(schema)
  if (options?.length) return cloneJson(options[0].value)
  switch (schemaType(schema)) {
    case "object":
      return {}
    case "array":
      return []
    case "boolean":
      return false
    case "integer":
    case "number":
      return numberKeyword(schema.minimum) ?? numberKeyword(schema.exclusiveMinimum) ?? 0
    case "string":
      return ""
    default:
      return null
  }
}

export function defaultArrayItem(root: JsonSchema, schema: JsonSchema, index: number): JsonValue {
  const itemSchema = arrayItemSchema(schema, index)
  return hydrateValue(root, itemSchema, undefined, true) ?? emptyValue(itemSchema)
}

export function arrayItemSchema(schema: JsonSchema, index: number): JsonSchema {
  const prefixItems = Array.isArray(schema.prefixItems) ? schema.prefixItems.map(asSchema) : []
  if (prefixItems[index]) return prefixItems[index]
  return asSchema(schema.items)
}

export function uiOptionsFor(
  uiSchema: JsonSchema,
  pointer: string,
  schema: JsonSchema,
): UiOptions {
  const segments = pointerSegments(pointer)
  let nested: unknown = uiSchema
  for (const segment of segments) {
    const object = asSchema(nested)
    nested = object[segment] ?? asSchema(object.properties)[segment]
  }
  const fields = asSchema(uiSchema.fields)
  const wildcard = wildcardUiOptions({ ...uiSchema, ...fields }, pointer)
  const direct = asSchema(uiSchema[pointer])
  const field = asSchema(fields[pointer])
  const inline = asSchema(schema["x-qimen-ui"])
  return normalizeUiOptions({ ...asSchema(nested), ...wildcard, ...direct, ...field, ...inline })
}

function wildcardUiOptions(entries: JsonSchema, pointer: string): JsonSchema {
  const actual = pointerSegments(pointer)
  return Object.entries(entries)
    .filter(([pattern, value]) => {
      if (!pattern.startsWith("/") || !pattern.includes("*") || !isPlainObject(value)) return false
      const expected = pointerSegments(pattern)
      return expected.length === actual.length
        && expected.every((segment, index) => segment === "*" || segment === actual[index])
    })
    .sort(([left], [right]) => left.localeCompare(right))
    .reduce((merged, [, value]) => ({ ...merged, ...asSchema(value) }), {})
}

function normalizeUiOptions(value: JsonSchema): UiOptions {
  const nestedOptions = asSchema(value["ui:options"])
  const merged = { ...nestedOptions, ...value }
  const labels = asSchema(merged.labels)
  return {
    widget: stringOption(merged.widget ?? merged["ui:widget"]),
    title: stringOption(merged.title ?? merged.label ?? merged["ui:title"]),
    description: stringOption(merged.description ?? merged["ui:description"]),
    placeholder: stringOption(merged.placeholder ?? merged["ui:placeholder"]),
    help: stringOption(merged.help ?? merged["ui:help"]),
    unit: stringOption(merged.unit),
    order: stringArray(merged.order ?? merged["ui:order"]),
    hidden: booleanOption(merged.hidden ?? merged["ui:hidden"]),
    disabled: booleanOption(merged.disabled ?? merged["ui:disabled"]),
    readonly: booleanOption(merged.readonly ?? merged.readOnly ?? merged["ui:readonly"]),
    rows: numberKeyword(merged.rows),
    step: numberKeyword(merged.step),
    columns: numberKeyword(merged.columns),
    width: stringOption(merged.width),
    addLabel: stringOption(merged.addLabel),
    itemLabel: stringOption(merged.itemLabel),
    itemTitle: stringOption(merged.itemTitle),
    emptyLabel: stringOption(merged.emptyLabel),
    labels: Object.keys(labels).length
      ? Object.fromEntries(Object.entries(labels).filter((entry): entry is [string, string] => typeof entry[1] === "string"))
      : undefined,
    collapsible: booleanOption(merged.collapsible),
    collapsed: booleanOption(merged.collapsed),
  }
}

export function orderedProperties(schema: JsonSchema, ui: UiOptions) {
  const names = Object.keys(asSchema(schema.properties))
  if (!ui.order?.length) return names
  const available = new Set(names)
  const ordered: string[] = []
  for (const name of ui.order) {
    if (name === "*") {
      for (const candidate of names) {
        if (available.delete(candidate)) ordered.push(candidate)
      }
    } else if (available.delete(name)) {
      ordered.push(name)
    }
  }
  for (const name of names) {
    if (available.delete(name)) ordered.push(name)
  }
  return ordered
}

export function propertySchema(schema: JsonSchema, name: string): JsonSchema {
  const declared = asSchema(asSchema(schema.properties)[name])
  if (Object.keys(declared).length > 0) return declared
  const patterns = asSchema(schema.patternProperties)
  for (const [pattern, child] of Object.entries(patterns)) {
    try {
      if (new RegExp(pattern).test(name)) return asSchema(child)
    } catch {
      // The backend rejects invalid regular expressions through JSON Schema compilation.
    }
  }
  return schema.additionalProperties === false ? {} : asSchema(schema.additionalProperties)
}

export type FormValidator = (value: JsonObject, secrets: SecretDraft) => FormValidationError[]

let schemaSequence = 0

/**
 * Hyperjump interprets JSON Schema instead of generating a Function at runtime.
 * The admin panel can therefore keep its strict CSP without losing local draft
 * validation for schemas supplied by third-party plugins.
 */
export async function compileFormValidator(schema: JsonSchema): Promise<FormValidator> {
  const uri = nextSchemaUri()
  let registered = false
  try {
    registerSchema(schema as never, uri)
    registered = true
    const validate = await compileSchema(uri)
    return (value, secrets) => validateFormValue(validate, schema, value, secrets)
  } catch (error) {
    const message = error instanceof Error ? error.message : "Schema 无法解析"
    return () => [{ pointer: "", message: `Schema 无法解析：${message}`, keyword: "schema" }]
  } finally {
    if (registered) unregisterSchema(uri)
  }
}

function nextSchemaUri() {
  schemaSequence += 1
  return `urn:qimenbot:plugin-config:${schemaSequence}`
}

function validateFormValue(
  validate: SchemaValidator,
  schema: JsonSchema,
  value: JsonObject,
  secrets: SecretDraft,
): FormValidationError[] {
  const validationValue = materializeSecrets(value, secrets)
  const output = validate(validationValue, { outputFormat: "DETAILED" })
  if (output.valid || !output.errors) return []

  const errors: FormValidationError[] = []
  const seen = new Set<string>()
  const visit = (unit: OutputUnit) => {
    const children = unit.errors ?? []
    if (children.length > 0) {
      // Keep a useful parent error for combinators, while still exposing the
      // concrete child fields when the interpreter reports them.
      const keyword = keywordName(unit.keyword)
      if (["oneOf", "anyOf", "not"].includes(keyword)) {
        addFormError(errors, seen, {
          pointer: instancePointer(unit.instanceLocation),
          keyword,
          message: localizedErrorMessage(keyword, schemaAtKeyword(schema, unit.absoluteKeywordLocation)),
        })
      }
      children.forEach(visit)
      return
    }

    const keyword = keywordName(unit.keyword)
    const pointer = instancePointer(unit.instanceLocation)
    const keywordSchema = schemaAtKeyword(schema, unit.absoluteKeywordLocation)
    if (keyword === "required") {
      const parent = getAtPointer(validationValue, pointer)
      const required = Array.isArray(keywordSchema.required)
        ? keywordSchema.required.filter((item): item is string => typeof item === "string")
        : []
      const missing = required.filter((name) => !isJsonObject(parent) || !(name in parent))
      for (const name of missing.length > 0 ? missing : [""]) {
        addFormError(errors, seen, {
          pointer: name ? joinPointer(pointer, name) : pointer,
          keyword,
          message: localizedErrorMessage(keyword, keywordSchema),
        })
      }
      return
    }
    if (keyword === "additionalProperties") {
      const parent = getAtPointer(validationValue, pointer)
      const properties = new Set(Object.keys(asSchema(keywordSchema.properties)))
      const patterns = Object.keys(asSchema(keywordSchema.patternProperties)).map((pattern) => {
        try { return new RegExp(pattern) } catch { return null }
      }).filter((pattern): pattern is RegExp => pattern !== null)
      if (isJsonObject(parent)) {
        for (const name of Object.keys(parent)) {
          if (!properties.has(name) && !patterns.some((pattern) => pattern.test(name))) {
            addFormError(errors, seen, {
              pointer: joinPointer(pointer, name),
              keyword,
              message: localizedErrorMessage(keyword, keywordSchema),
            })
          }
        }
      }
      return
    }

    addFormError(errors, seen, {
      pointer,
      keyword,
      message: localizedErrorMessage(keyword, keywordSchema),
    })
  }
  output.errors.forEach(visit)
  return errors.filter((error) => !isPreservedSecretError(error.pointer, secrets))
}

function addFormError(
  errors: FormValidationError[],
  seen: Set<string>,
  error: FormValidationError,
) {
  const key = `${error.pointer}\u0000${error.keyword}`
  if (seen.has(key)) return
  seen.add(key)
  errors.push(error)
}

function keywordName(keyword: string) {
  const normalized = keyword.split("/").at(-1) ?? keyword
  return decodeURIComponent(normalized)
}

function instancePointer(location: string) {
  const hash = location.indexOf("#")
  const fragment = hash >= 0 ? location.slice(hash + 1) : location
  if (!fragment || fragment === "/") return ""
  try {
    return decodeURIComponent(fragment)
  } catch {
    return fragment
  }
}

function schemaAtKeyword(schema: JsonSchema, location: string): JsonSchema {
  const hash = location.indexOf("#")
  const fragment = hash >= 0 ? location.slice(hash + 1) : ""
  const segments = fragment.split("/").filter(Boolean)
  segments.pop()
  const pointer = segments.length > 0 ? "/" + segments.join("/") : ""
  return asSchema(getAtPointer(schema as unknown as JsonValue, decodePointer(pointer)))
}

function decodePointer(pointer: string) {
  try {
    return decodeURIComponent(pointer)
  } catch {
    return pointer
  }
}

function materializeSecrets(value: JsonObject, secrets: SecretDraft) {
  let next = cloneJson(value)
  for (const [pointer, entry] of Object.entries(secrets)) {
    if (entry.update === null) {
      next = removeAtPointer(next, pointer)
    } else if (typeof entry.update === "string") {
      next = setAtPointer(next, pointer, entry.update)
    } else if (entry.source) {
      next = setAtPointer(next, pointer, "__qimen_preserved_secret__")
    }
  }
  return next
}

function isPreservedSecretError(pointer: string, secrets: SecretDraft) {
  return Object.entries(secrets).some(([secretPointer, entry]) => {
    if (!entry.source || entry.update !== undefined) return false
    return pointer === secretPointer || pointer.startsWith(secretPointer + "/")
  })
}

function localizedErrorMessage(keyword: string, schema: JsonSchema) {
  switch (keyword) {
    case "required":
      return "此项为必填项"
    case "type":
      return `类型应为 ${Array.isArray(schema.type) ? schema.type.join(" / ") : String(schema.type ?? "指定类型")}`
    case "format":
      return `格式应为 ${String(schema.format ?? "指定格式")}`
    case "enum":
      return "请选择允许的值"
    case "const":
      return "值与插件要求不一致"
    case "minLength":
      return `至少输入 ${String(schema.minLength ?? "指定长度")} 个字符`
    case "maxLength":
      return `最多输入 ${String(schema.maxLength ?? "指定长度")} 个字符`
    case "minimum":
      return `不能小于 ${String(schema.minimum ?? "指定值")}`
    case "maximum":
      return `不能大于 ${String(schema.maximum ?? "指定值")}`
    case "exclusiveMinimum":
      return `必须大于 ${String(schema.exclusiveMinimum ?? "指定值")}`
    case "exclusiveMaximum":
      return `必须小于 ${String(schema.exclusiveMaximum ?? "指定值")}`
    case "multipleOf":
      return `必须是 ${String(schema.multipleOf ?? "指定值")} 的倍数`
    case "minItems":
      return `至少需要 ${String(schema.minItems ?? "指定数量")} 项`
    case "maxItems":
      return `最多允许 ${String(schema.maxItems ?? "指定数量")} 项`
    case "uniqueItems":
      return "数组项不能重复"
    case "pattern":
      return "内容不符合插件声明的格式"
    case "additionalProperties":
      return "此配置项不在插件 Schema 中"
    case "oneOf":
      return "配置必须且只能匹配一种形式"
    case "anyOf":
      return "配置未匹配任何允许的形式"
    default:
      return "配置不符合 Schema"
  }
}

export function errorsForPointer(errors: FormValidationError[], pointer: string) {
  return errors.filter((error) => error.pointer === pointer)
}

export function childErrorCount(errors: FormValidationError[], pointer: string) {
  const prefix = pointer ? pointer + "/" : "/"
  return errors.filter((error) => error.pointer.startsWith(prefix)).length
}

export function remapSecretsAfterArrayOperation(
  secrets: SecretDraft,
  arrayPointer: string,
  operation: { type: "remove"; index: number } | { type: "move"; from: number; to: number },
): SecretDraft {
  const prefix = arrayPointer + "/"
  const result: SecretDraft = {}
  for (const [pointer, entry] of Object.entries(secrets)) {
    if (!pointer.startsWith(prefix)) {
      result[pointer] = entry
      continue
    }
    const suffix = pointer.slice(prefix.length)
    const separator = suffix.indexOf("/")
    const rawIndex = separator >= 0 ? suffix.slice(0, separator) : suffix
    const index = Number(rawIndex)
    if (!Number.isInteger(index)) {
      result[pointer] = entry
      continue
    }
    let nextIndex = index
    if (operation.type === "remove") {
      if (index === operation.index) continue
      if (index > operation.index) nextIndex -= 1
    } else if (index === operation.from) {
      nextIndex = operation.to
    } else if (operation.from < operation.to && index > operation.from && index <= operation.to) {
      nextIndex -= 1
    } else if (operation.from > operation.to && index >= operation.to && index < operation.from) {
      nextIndex += 1
    }
    const rest = separator >= 0 ? suffix.slice(separator) : ""
    result[prefix + nextIndex + rest] = entry
  }
  return result
}

export function humanizeProperty(name: string) {
  return name
    .replace(/[_-]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/^./, (character) => character.toUpperCase())
}

export function displayJsonValue(value: JsonValue) {
  if (value === null) return "null"
  if (typeof value === "boolean") return value ? "是" : "否"
  if (typeof value === "string") return value
  return JSON.stringify(value)
}

export function numberKeyword(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function stringOption(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function booleanOption(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined
}

function stringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : undefined
}

function isJsonValue(value: unknown): value is JsonValue {
  if (value === null || ["string", "number", "boolean"].includes(typeof value)) return true
  if (Array.isArray(value)) return value.every(isJsonValue)
  return isPlainObject(value) && Object.values(value).every(isJsonValue)
}
