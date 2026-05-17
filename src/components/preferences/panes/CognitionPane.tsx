// @ts-nocheck — references legacy load/save/list cognition commands that
// were consolidated into the unified `preferences.toml` flow
// (`get_preferences` / `set_cognition_settings`). Pane stays mounted
// at runtime (calls just resolve to error and render the error path)
// but is being rewritten as a follow-up slice. See AGENTS.md.
//
// Settings → Cognition. Opt-in surface for the contextification
// background flow (see pitch
// `equanimitech/docs/pitches/2026-05-06-contextification-background-flow.md`).
//
// The principal picks a provider — Anthropic Messages API (BYOK), or
// any OpenAI Chat-Completions endpoint (Ollama for sovereign / on-
// device, OpenRouter / OpenAI / Together for cloud). The Refresh
// button calls the provider's `/models` endpoint and populates the
// model dropdown so the principal doesn't have to memorize identifiers.
//
// Default-off discipline: this pane *creates* `~/.secretariat/cognition.json`
// when saved. Until then, contextification stays a no-op — captures
// land where they were filed and stay there.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { RefreshCw, Save } from 'lucide-react'
import { commands, type CognitionConfigDto } from '@/lib/bindings'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

type Provider = 'anthropic' | 'openai-compat'

interface FormState {
  provider: Provider
  apiKey: string
  apiBase: string
  model: string
  routeThreshold: number
}

const DEFAULT_FORM: FormState = {
  provider: 'anthropic',
  apiKey: '',
  apiBase: '',
  model: '',
  routeThreshold: 0.7,
}

export function CognitionPane() {
  const [form, setForm] = useState<FormState>(DEFAULT_FORM)
  const [models, setModels] = useState<string[]>([])
  const [loadingConfig, setLoadingConfig] = useState(true)
  const [savedNote, setSavedNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [refreshingModels, setRefreshingModels] = useState(false)
  const [saving, setSaving] = useState(false)

  // Hydrate the form from disk on mount. Empty when no config saved
  // yet — the principal sees the default-off state explicitly.
  // Wrapped in try/finally because the legacy IPC commands referenced
  // here were consolidated into the unified preferences flow and now
  // throw (not resolve to {status:'error'}); without the finally the
  // spinner hangs forever. Rewriting against `get_preferences` is
  // the followup; this just stops the hang.
  useEffect(() => {
    void (async () => {
      try {
        const result = await commands.loadCognitionConfig()
        if (result.status === 'ok' && result.data) {
          setForm(dtoToForm(result.data))
        } else if (result.status === 'error') {
          setError(result.error)
        }
      } catch (e) {
        setError(String(e))
      } finally {
        setLoadingConfig(false)
      }
    })()
  }, [])

  const dto = useMemo<CognitionConfigDto>(() => formToDto(form), [form])

  const handleRefreshModels = useCallback(async () => {
    setError(null)
    setRefreshingModels(true)
    try {
      const result = await commands.listCognitionModels(dto)
      if (result.status === 'ok') {
        setModels(result.data)
        // Auto-pick the first model when the principal's current
        // selection doesn't appear in the fetched list — saves a click.
        if (result.data.length > 0 && !result.data.includes(form.model)) {
          setForm(prev => ({ ...prev, model: result.data[0] ?? prev.model }))
        }
      } else {
        setError(result.error)
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setRefreshingModels(false)
    }
  }, [dto, form.model])

  const handleSave = useCallback(async () => {
    setError(null)
    setSaving(true)
    try {
      const result = await commands.saveCognitionConfig(dto)
      if (result.status === 'error') {
        setError(result.error)
        return
      }
      setSavedNote('Saved.')
      setTimeout(() => setSavedNote(null), 2000)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }, [dto])

  const apiBasePlaceholder =
    form.provider === 'anthropic'
      ? 'https://api.anthropic.com (default)'
      : 'http://localhost:11434/v1   |   https://openrouter.ai/api/v1'

  const apiKeyHidden = form.provider === 'openai-compat' && isOllamaBase(form.apiBase)
  const apiKeyHelper = apiKeyHidden
    ? "Ollama doesn't need a key."
    : form.provider === 'anthropic'
      ? 'BYOK Anthropic key. Stays on this device.'
      : 'BYOK provider key. Stays on this device.'

  if (loadingConfig) {
    return <div className="p-2 text-sm text-muted-foreground">Loading…</div>
  }

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">Cognition substrate</Label>
          <p className="text-xs text-muted-foreground">
            Pick where queue-routing suggestions come from. Default-off:
            until you save here, captures stay where they were filed.
          </p>
        </div>

        <RadioGroup
          value={form.provider}
          onValueChange={value =>
            setForm(prev => ({ ...prev, provider: value as Provider, model: '' }))
          }
          className="grid gap-2"
        >
          <label className="flex cursor-pointer items-start gap-3 rounded-md border p-3 hover:bg-muted/30">
            <RadioGroupItem value="anthropic" id="prov-anthropic" className="mt-1" />
            <div className="space-y-0.5">
              <div className="text-sm font-medium">Anthropic (Claude)</div>
              <p className="text-xs text-muted-foreground">
                BYOK Anthropic key. Capture body sent to{' '}
                <code>api.anthropic.com</code>.
              </p>
            </div>
          </label>
          <label className="flex cursor-pointer items-start gap-3 rounded-md border p-3 hover:bg-muted/30">
            <RadioGroupItem value="openai-compat" id="prov-oai" className="mt-1" />
            <div className="space-y-0.5">
              <div className="text-sm font-medium">OpenAI-compatible</div>
              <p className="text-xs text-muted-foreground">
                Ollama (local — never leaves device), OpenRouter, OpenAI,
                Together, Groq. Anything that speaks Chat Completions.
              </p>
            </div>
          </label>
        </RadioGroup>
      </section>

      <section className="space-y-4 border-t pt-4">
        {form.provider === 'openai-compat' && (
          <div className="space-y-1.5 max-w-md">
            <Label htmlFor="api-base" className="text-sm font-medium">
              Base URL
            </Label>
            <Input
              id="api-base"
              type="url"
              placeholder={apiBasePlaceholder}
              value={form.apiBase}
              onChange={e => setForm(prev => ({ ...prev, apiBase: e.target.value }))}
            />
            <p className="text-xs text-muted-foreground">
              Ollama: <code>http://localhost:11434/v1</code>. OpenRouter:{' '}
              <code>https://openrouter.ai/api/v1</code>.
            </p>
          </div>
        )}

        {!apiKeyHidden && (
          <div className="space-y-1.5 max-w-md">
            <Label htmlFor="api-key" className="text-sm font-medium">
              API key
            </Label>
            <Input
              id="api-key"
              type="password"
              autoComplete="off"
              placeholder={form.provider === 'anthropic' ? 'sk-ant-…' : 'sk-or-… or sk-…'}
              value={form.apiKey}
              onChange={e => setForm(prev => ({ ...prev, apiKey: e.target.value }))}
            />
            <p className="text-xs text-muted-foreground">{apiKeyHelper}</p>
          </div>
        )}
        {apiKeyHidden && (
          <p className="max-w-md text-xs italic text-muted-foreground">
            {apiKeyHelper}
          </p>
        )}

        <div className="space-y-1.5 max-w-md">
          <div className="flex items-end justify-between gap-2">
            <Label htmlFor="model" className="text-sm font-medium">
              Model
            </Label>
            <button
              type="button"
              onClick={handleRefreshModels}
              disabled={refreshingModels}
              className="inline-flex items-center gap-1 rounded-md border bg-muted px-2 py-1 text-xs hover:bg-muted/70 disabled:opacity-50"
            >
              <RefreshCw
                className={`h-3 w-3 ${refreshingModels ? 'animate-spin' : ''}`}
              />
              {refreshingModels ? 'Refreshing…' : 'Refresh'}
            </button>
          </div>
          {models.length > 0 ? (
            <select
              id="model"
              value={form.model}
              onChange={e => setForm(prev => ({ ...prev, model: e.target.value }))}
              className="w-full rounded-md border bg-background px-2 py-1.5 text-sm"
            >
              {models.map(m => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          ) : (
            <Input
              id="model"
              type="text"
              placeholder={
                form.provider === 'anthropic'
                  ? 'claude-haiku-4-5-20251001'
                  : 'llama3.1:8b   |   gpt-4o-mini'
              }
              value={form.model}
              onChange={e => setForm(prev => ({ ...prev, model: e.target.value }))}
            />
          )}
          <p className="text-xs text-muted-foreground">
            Click Refresh to fetch the available models from the provider.
          </p>
        </div>

        <div className="space-y-1.5 max-w-md">
          <div className="flex items-center justify-between">
            <Label htmlFor="threshold" className="text-sm font-medium">
              Confidence threshold
            </Label>
            <span className="text-sm tabular-nums text-muted-foreground">
              {form.routeThreshold.toFixed(2)}
            </span>
          </div>
          <input
            id="threshold"
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={form.routeThreshold}
            onChange={e =>
              setForm(prev => ({
                ...prev,
                routeThreshold: parseFloat(e.target.value),
              }))
            }
            className="w-full"
          />
          <p className="text-xs text-muted-foreground">
            Suggestions below this score don&apos;t apply — the capture stays
            in <code>inbox:triage</code> for you to file by hand.
          </p>
        </div>
      </section>

      <section className="flex items-center gap-3 border-t pt-4">
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="inline-flex items-center gap-1.5 rounded-md border bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
        >
          <Save className="h-3.5 w-3.5" />
          {saving ? 'Saving…' : 'Save'}
        </button>
        {savedNote && (
          <span className="text-xs text-emerald-600 dark:text-emerald-400">
            {savedNote}
          </span>
        )}
        {error && (
          <div className="rounded-md border border-destructive bg-destructive/10 p-2 text-xs text-destructive">
            {error}
          </div>
        )}
      </section>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers — DTO ↔ form conversions, Ollama-base detection.
// ---------------------------------------------------------------------------

function dtoToForm(dto: CognitionConfigDto): FormState {
  const provider: Provider = dto.provider === 'openai-compat' ? 'openai-compat' : 'anthropic'
  return {
    provider,
    apiKey: dto.api_key ?? '',
    apiBase: dto.api_base ?? '',
    model: dto.model ?? '',
    routeThreshold: dto.route_threshold ?? 0.7,
  }
}

function formToDto(form: FormState): CognitionConfigDto {
  return {
    provider: form.provider,
    api_key: form.apiKey === '' ? null : form.apiKey,
    api_base: form.apiBase === '' ? null : form.apiBase,
    model: form.model === '' ? null : form.model,
    route_threshold: form.routeThreshold,
  }
}

/// Detect a localhost-shaped base URL so the API-key field can hide
/// itself for Ollama. Treat anything pointing at a private/loopback
/// address as keyless. The key still works if the principal sets one;
/// this is just UI affordance.
function isOllamaBase(apiBase: string): boolean {
  const trimmed = apiBase.trim().toLowerCase()
  if (trimmed === '') return false
  return (
    trimmed.includes('localhost') ||
    trimmed.includes('127.0.0.1') ||
    trimmed.includes('0.0.0.0') ||
    trimmed.startsWith('http://[::1]')
  )
}
