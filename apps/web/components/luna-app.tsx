'use client'

import {
  ArrowLeft,
  Camera,
  CircleStop,
  Mic,
  Moon,
  Paperclip,
  Plus,
  Search,
  Send,
  Sun,
  X,
} from 'lucide-react'
import type {
  Attachment,
  AttachmentResponse,
  Bootstrap,
  Conversation,
  ConversationMessages,
  Message,
  PairingExchangeResponse,
  SendMessageResponse,
  TranscriptionResponse,
} from '@luna/protocol'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { applyServerEvent, type LunaClientState, type LunaEvent } from '../lib/events.js'

type Phase = 'loading' | 'pairing' | 'ready'
type Theme = 'latte' | 'mocha'

const initialState: LunaClientState = {
  conversations: [],
  messages: [],
  selectedConversationId: undefined,
  cursor: 0,
}

export function LunaApp() {
  const [phase, setPhase] = useState<Phase>('loading')
  const [client, setClient] = useState(initialState)
  const [error, setError] = useState<string>()
  const [search, setSearch] = useState('')
  const [theme, setTheme] = useState<Theme>('latte')
  const cursor = useRef(0)

  const installBootstrap = useCallback((bootstrap: Bootstrap) => {
    cursor.current = bootstrap.cursor
    setClient({
      conversations: bootstrap.conversations,
      messages: [],
      selectedConversationId: bootstrap.conversations[0]?.id,
      cursor: bootstrap.cursor,
    })
    setPhase('ready')
  }, [])

  useEffect(() => {
    const stored = window.localStorage.getItem('luna-theme')
    const next: Theme =
      stored === 'latte' || stored === 'mocha'
        ? stored
        : window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'mocha'
          : 'latte'
    setTheme(next)
    document.documentElement.dataset.theme = next
    void api<Bootstrap>('/v1/bootstrap')
      .then(installBootstrap)
      .catch((requestError: unknown) => {
        if (requestError instanceof ApiFailure && requestError.status === 401) setPhase('pairing')
        else {
          setError(messageFromError(requestError))
          setPhase('pairing')
        }
      })
    if ('serviceWorker' in navigator) void navigator.serviceWorker.register('/sw.js')
  }, [installBootstrap])

  useEffect(() => {
    if (phase !== 'ready') return
    let socket: WebSocket | undefined
    let retry: ReturnType<typeof setTimeout> | undefined
    let stopped = false
    const connect = () => {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
      socket = new WebSocket(
        `${protocol}//${window.location.host}/v1/events?after=${String(cursor.current)}`,
      )
      socket.onmessage = (message) => {
        const event = JSON.parse(String(message.data)) as LunaEvent
        if (event.type === 'server.welcome' || event.type === 'server.pong') return
        setClient((current) => {
          const next = applyServerEvent(current, event)
          cursor.current = next.cursor
          return next
        })
      }
      socket.onclose = () => {
        if (!stopped) retry = setTimeout(connect, 1_000)
      }
    }
    connect()
    return () => {
      stopped = true
      if (retry) clearTimeout(retry)
      socket?.close()
    }
  }, [phase])

  useEffect(() => {
    const conversationId = client.selectedConversationId
    if (!conversationId || phase !== 'ready') return
    let active = true
    void api<ConversationMessages>(`/v1/conversations/${conversationId}/messages`)
      .then((response) => {
        if (active) setClient((current) => ({ ...current, messages: response.messages }))
      })
      .catch((requestError: unknown) => setError(messageFromError(requestError)))
    return () => {
      active = false
    }
  }, [client.selectedConversationId, phase])

  const setSelectedConversation = (conversationId: string | undefined) => {
    setClient((current) => ({
      ...current,
      messages: [],
      selectedConversationId: conversationId,
    }))
  }

  const createConversation = async () => {
    try {
      const conversation = await api<Conversation>('/v1/conversations', {
        method: 'POST',
        body: JSON.stringify({}),
      })
      setClient((current) => ({
        ...current,
        conversations: [conversation, ...current.conversations],
        selectedConversationId: conversation.id,
        messages: [],
      }))
    } catch (requestError) {
      setError(messageFromError(requestError))
    }
  }

  const selectTheme = (next: Theme) => {
    setTheme(next)
    window.localStorage.setItem('luna-theme', next)
    document.documentElement.dataset.theme = next
  }

  if (phase === 'loading') return <LoadingScreen />
  if (phase === 'pairing') {
    return <PairingScreen error={error} onPaired={installBootstrap} onError={setError} />
  }

  const selected = client.conversations.find(
    (conversation) => conversation.id === client.selectedConversationId,
  )
  const filtered = client.conversations.filter((conversation) =>
    conversation.title.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
  )

  return (
    <main className="app-shell">
      <aside className={`sidebar ${selected ? 'mobile-hidden' : ''}`}>
        <header className="sidebar-header">
          <div>
            <p className="eyebrow">Persistent Pi</p>
            <h1>Luna</h1>
          </div>
          <button
            className="icon-button accent"
            aria-label="New conversation"
            onClick={() => void createConversation()}
          >
            <Plus size={19} />
          </button>
        </header>
        <label className="search-field">
          <Search size={15} />
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search conversations"
          />
        </label>
        <nav className="conversation-list" aria-label="Conversations">
          {filtered.map((conversation) => (
            <ConversationCell
              key={conversation.id}
              conversation={conversation}
              selected={conversation.id === selected?.id}
              onSelect={() => setSelectedConversation(conversation.id)}
            />
          ))}
          {filtered.length === 0 && (
            <div className="empty-list">
              <p>No conversations yet.</p>
              <button onClick={() => void createConversation()}>Start one</button>
            </div>
          )}
        </nav>
        <footer className="sidebar-footer">
          <span>{theme === 'latte' ? 'Catppuccin Latte' : 'Catppuccin Mocha'}</span>
          <button
            className="icon-button"
            aria-label="Toggle theme"
            onClick={() => selectTheme(theme === 'latte' ? 'mocha' : 'latte')}
          >
            {theme === 'latte' ? <Moon size={17} /> : <Sun size={17} />}
          </button>
        </footer>
      </aside>
      <section className={`conversation-panel ${selected ? '' : 'mobile-hidden'}`}>
        {selected ? (
          <ConversationView
            conversation={selected}
            messages={client.messages}
            onBack={() => setSelectedConversation(undefined)}
            onMessage={(message) =>
              setClient((current) => ({
                ...current,
                messages: upsertMessage(current.messages, message),
              }))
            }
            onRename={(conversation) =>
              setClient((current) => ({
                ...current,
                conversations: current.conversations.map((item) =>
                  item.id === conversation.id ? conversation : item,
                ),
              }))
            }
            onError={setError}
          />
        ) : (
          <Welcome onCreate={() => void createConversation()} />
        )}
      </section>
      {error && (
        <button className="error-toast" onClick={() => setError(undefined)}>
          <span>{error}</span>
          <X size={15} />
        </button>
      )}
    </main>
  )
}

function PairingScreen({
  error,
  onPaired,
  onError,
}: {
  error: string | undefined
  onPaired: (bootstrap: Bootstrap) => void
  onError: (message: string | undefined) => void
}) {
  const [code, setCode] = useState('')
  const [deviceName, setDeviceName] = useState('Web app')
  const [submitting, setSubmitting] = useState(false)
  const pair = async (event: React.FormEvent) => {
    event.preventDefault()
    setSubmitting(true)
    onError(undefined)
    try {
      const response = await api<PairingExchangeResponse>('/v1/pairing/exchange', {
        method: 'POST',
        body: JSON.stringify({ code, deviceName, platform: 'web' }),
      })
      onPaired(response.bootstrap)
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setSubmitting(false)
    }
  }
  return (
    <main className="pairing-page">
      <section className="pairing-card">
        <div className="moon-mark" aria-hidden="true">
          ☾
        </div>
        <p className="eyebrow">Private by design</p>
        <h1>Pair with Luna</h1>
        <p>
          Enter the one-time code shown by your Luna server. Your conversations stay on your Mac.
        </p>
        <form onSubmit={(event) => void pair(event)}>
          <label>
            Pairing code
            <input
              autoFocus
              autoCapitalize="characters"
              autoComplete="one-time-code"
              value={code}
              onChange={(event) => setCode(event.target.value.replaceAll(/[^a-zA-Z0-9]/g, ''))}
              placeholder="A1B2C3D4E5"
              minLength={6}
              required
            />
          </label>
          <label>
            Device name
            <input
              value={deviceName}
              onChange={(event) => setDeviceName(event.target.value)}
              required
            />
          </label>
          <button className="primary-button" disabled={submitting}>
            {submitting ? 'Pairing…' : 'Pair device'}
          </button>
        </form>
        {error && <p className="form-error">{error}</p>}
      </section>
    </main>
  )
}

function ConversationCell({
  conversation,
  selected,
  onSelect,
}: {
  conversation: Conversation
  selected: boolean
  onSelect: () => void
}) {
  const repo = conversation.repositories[0]
  return (
    <button className={`conversation-cell ${selected ? 'selected' : ''}`} onClick={onSelect}>
      <span className="avatar">{repo?.icon.fallbackText ?? '☾'}</span>
      <span className="cell-copy">
        <span className="cell-title">{conversation.title}</span>
        <span className="cell-preview">
          {conversation.preview || stateLabel(conversation.state)}
        </span>
      </span>
      <span
        className={`state-dot ${conversation.state}`}
        aria-label={stateLabel(conversation.state)}
      />
    </button>
  )
}

function ConversationView({
  conversation,
  messages,
  onBack,
  onMessage,
  onRename,
  onError,
}: {
  conversation: Conversation
  messages: Message[]
  onBack: () => void
  onMessage: (message: Message) => void
  onRename: (conversation: Conversation) => void
  onError: (message: string | undefined) => void
}) {
  const end = useRef<HTMLDivElement>(null)
  const busy = ['working', 'compacting', 'retrying', 'restoring', 'starting'].includes(
    conversation.state,
  )
  useEffect(() => {
    end.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const rename = async () => {
    const title = window.prompt('Conversation title', conversation.title)?.trim()
    if (!title || title === conversation.title) return
    try {
      onRename(
        await api<Conversation>(`/v1/conversations/${conversation.id}`, {
          method: 'PATCH',
          body: JSON.stringify({ title }),
        }),
      )
    } catch (requestError) {
      onError(messageFromError(requestError))
    }
  }

  return (
    <div className="conversation-view">
      <header className="conversation-header">
        <button className="icon-button back-button" aria-label="Back" onClick={onBack}>
          <ArrowLeft size={19} />
        </button>
        <button className="title-button" onClick={() => void rename()} title="Rename conversation">
          <strong>{conversation.title}</strong>
          <span>
            {conversation.repositories.map((repository) => repository.displayName).join(' · ') ||
              'Home'}
          </span>
        </button>
        <span className={`status-pill ${busy ? 'active' : ''}`}>
          <span /> {stateLabel(conversation.state)}
        </span>
      </header>
      <div className="message-scroll">
        {messages.length === 0 ? (
          <div className="conversation-empty">
            <div className="moon-mark">☾</div>
            <h2>What should we work on?</h2>
            <p>
              Luna starts every conversation at your home directory and follows Pi across
              repositories.
            </p>
          </div>
        ) : (
          messages.map((message) => <MessageBubble key={message.id} message={message} />)
        )}
        {busy && !messages.some((message) => message.status === 'streaming') && <TypingIndicator />}
        <div ref={end} />
      </div>
      <Composer conversation={conversation} busy={busy} onMessage={onMessage} onError={onError} />
    </div>
  )
}

function MessageBubble({ message }: { message: Message }) {
  return (
    <article className={`message-row ${message.role}`}>
      <div className="message-bubble">
        {message.attachments.length > 0 && (
          <div className="attachment-grid">
            {message.attachments.map((attachment) => (
              // The server requires the paired cookie for every image request.
              <img key={attachment.id} src={attachment.contentUrl} alt={attachment.fileName} />
            ))}
          </div>
        )}
        {message.role === 'assistant' ? (
          <div className="markdown">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.text}</ReactMarkdown>
            {message.status === 'streaming' && <span className="stream-caret" />}
          </div>
        ) : (
          <p>{message.text}</p>
        )}
      </div>
    </article>
  )
}

function Composer({
  conversation,
  busy,
  onMessage,
  onError,
}: {
  conversation: Conversation
  busy: boolean
  onMessage: (message: Message) => void
  onError: (message: string | undefined) => void
}) {
  const [text, setText] = useState('')
  const [files, setFiles] = useState<File[]>([])
  const [sending, setSending] = useState(false)
  const [recording, setRecording] = useState(false)
  const recorder = useRef<MediaRecorder | undefined>(undefined)
  const chunks = useRef<Blob[]>([])
  const fileInput = useRef<HTMLInputElement>(null)
  const cameraInput = useRef<HTMLInputElement>(null)

  const previews = useMemo(
    () => files.map((file) => ({ file, url: URL.createObjectURL(file) })),
    [files],
  )
  useEffect(
    () => () => {
      for (const preview of previews) URL.revokeObjectURL(preview.url)
    },
    [previews],
  )

  const addFiles = (incoming: File[]) => {
    setFiles((current) =>
      [...current, ...incoming.filter((file) => file.type.startsWith('image/'))].slice(0, 6),
    )
  }

  const sendMessage = async () => {
    const trimmed = text.trim()
    if ((!trimmed && files.length === 0) || sending) return
    setSending(true)
    onError(undefined)
    try {
      const attachments: Attachment[] = []
      for (const file of files) {
        const body = new FormData()
        body.append('conversationId', conversation.id)
        body.append('file', file)
        const uploaded = await api<AttachmentResponse>('/v1/attachments', {
          method: 'POST',
          body,
        })
        attachments.push(uploaded.attachment)
      }
      const response = await api<SendMessageResponse>(
        `/v1/conversations/${conversation.id}/messages`,
        {
          method: 'POST',
          body: JSON.stringify({
            clientMessageId: crypto.randomUUID(),
            text: trimmed || 'Please review the attached image.',
            attachmentIds: attachments.map((attachment) => attachment.id),
          }),
        },
      )
      onMessage(response.message)
      setText('')
      setFiles([])
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setSending(false)
    }
  }

  const abort = async () => {
    try {
      await api<void>(`/v1/conversations/${conversation.id}/abort`, { method: 'POST' })
    } catch (requestError) {
      onError(messageFromError(requestError))
    }
  }

  const toggleRecording = async () => {
    if (recording) {
      recorder.current?.stop()
      setRecording(false)
      return
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      chunks.current = []
      const next = new MediaRecorder(stream)
      recorder.current = next
      next.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.current.push(event.data)
      }
      next.onstop = () => {
        for (const track of stream.getTracks()) track.stop()
        const blob = new Blob(chunks.current, { type: next.mimeType || 'audio/webm' })
        const body = new FormData()
        body.append('file', blob, 'recording.webm')
        void api<TranscriptionResponse>('/v1/transcriptions', { method: 'POST', body })
          .then((response) =>
            setText((current) => `${current}${current ? ' ' : ''}${response.text}`),
          )
          .catch((requestError: unknown) => onError(messageFromError(requestError)))
      }
      next.start()
      setRecording(true)
    } catch (requestError) {
      onError(messageFromError(requestError))
    }
  }

  return (
    <div className="composer-wrap">
      {previews.length > 0 && (
        <div className="composer-previews">
          {previews.map(({ file, url }) => (
            <span key={`${file.name}-${String(file.lastModified)}`}>
              <img src={url} alt="Pending attachment" />
              <button
                onClick={() => setFiles((current) => current.filter((item) => item !== file))}
              >
                <X size={12} />
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="composer">
        <button
          className="icon-button"
          aria-label="Attach image"
          onClick={() => fileInput.current?.click()}
        >
          <Paperclip size={18} />
        </button>
        <button
          className="icon-button camera-action"
          aria-label="Take photo"
          onClick={() => cameraInput.current?.click()}
        >
          <Camera size={18} />
        </button>
        <textarea
          value={text}
          rows={1}
          placeholder={busy ? 'Steer Pi…' : 'Message Luna…'}
          onChange={(event) => setText(event.target.value)}
          onPaste={(event) => addFiles(Array.from(event.clipboardData.files))}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault()
              void sendMessage()
            }
          }}
        />
        {busy ? (
          <button
            className="icon-button stop-action"
            aria-label="Interrupt Pi"
            onClick={() => void abort()}
          >
            <CircleStop size={19} />
          </button>
        ) : (
          <button
            className={`icon-button ${recording ? 'recording' : ''}`}
            aria-label={recording ? 'Stop recording' : 'Transcribe voice'}
            onClick={() => void toggleRecording()}
          >
            <Mic size={18} />
          </button>
        )}
        <button
          className="send-button"
          aria-label="Send"
          disabled={sending}
          onClick={() => void sendMessage()}
        >
          {sending ? <span className="button-spinner" /> : <Send size={17} />}
        </button>
        <input
          ref={fileInput}
          hidden
          type="file"
          accept="image/png,image/jpeg,image/gif,image/webp"
          multiple
          onChange={(event) => addFiles(Array.from(event.target.files ?? []))}
        />
        <input
          ref={cameraInput}
          hidden
          type="file"
          accept="image/*"
          capture="environment"
          onChange={(event) => addFiles(Array.from(event.target.files ?? []))}
        />
      </div>
      <p className="composer-note">
        {conversation.activeWorkingDirectory.replace(/^\/Users\/[^/]+/, '~')}
      </p>
    </div>
  )
}

function TypingIndicator() {
  return (
    <div className="message-row assistant">
      <div className="typing" aria-label="Pi is working">
        <span />
        <span />
        <span />
      </div>
    </div>
  )
}

function Welcome({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="welcome">
      <div className="moon-mark">☾</div>
      <p className="eyebrow">Your work, in conversation</p>
      <h2>
        Powerful agents.
        <br />
        Familiar conversations.
      </h2>
      <p>Continue a Pi session from iPhone, iPad, or the web without losing context.</p>
      <button className="primary-button" onClick={onCreate}>
        <Plus size={17} /> New conversation
      </button>
    </div>
  )
}

function LoadingScreen() {
  return (
    <main className="loading-screen">
      <div className="moon-mark">☾</div>
      <span className="button-spinner" />
    </main>
  )
}

class ApiFailure extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.body && typeof init.body === 'string') headers.set('content-type', 'application/json')
  const response = await fetch(path, { ...init, headers, credentials: 'include' })
  if (!response.ok) {
    const body = (await response.json().catch(() => undefined)) as
      { message?: string; error?: { message?: string } } | undefined
    throw new ApiFailure(
      response.status,
      body?.message ?? body?.error?.message ?? 'Luna could not complete the request.',
    )
  }
  if (response.status === 204) return undefined as T
  return (await response.json()) as T
}

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : 'Luna could not complete the request.'
}

function stateLabel(state: Conversation['state']): string {
  return {
    creating: 'Creating',
    starting: 'Starting',
    idle: 'Ready',
    working: 'Working',
    compacting: 'Compacting',
    retrying: 'Retrying',
    crashed: 'Needs restore',
    restoring: 'Restoring',
    interrupted: 'Interrupted',
    stopped: 'Stopped',
    error: 'Needs attention',
  }[state]
}

function upsertMessage(messages: Message[], message: Message): Message[] {
  const found = messages.some((item) => item.id === message.id)
  return found
    ? messages.map((item) => (item.id === message.id ? message : item))
    : [...messages, message]
}
