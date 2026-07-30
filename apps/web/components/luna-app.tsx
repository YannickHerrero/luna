'use client'

import {
  Archive,
  ArrowLeft,
  Camera,
  Check,
  ChevronDown,
  Circle,
  CircleStop,
  ListChecks,
  Mic,
  Minus,
  Moon,
  Paperclip,
  Plus,
  Search,
  Send,
  Sun,
  TriangleAlert,
  X,
} from 'lucide-react'
import type {
  AgentActivity,
  AgentTaskList,
  Attachment,
  AttachmentResponse,
  Bootstrap,
  Conversation,
  ConversationMessages,
  Message,
  PairingCodeRequestResponse,
  PairingExchangeResponse,
  SendMessageResponse,
  TranscriptionResponse,
} from '@luna/protocol'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import remarkGfm from 'remark-gfm'
import { applyLatestMessage, sortConversations, upsertConversation } from '../lib/conversations.js'
import { applyServerEvent, type LunaClientState, type LunaEvent } from '../lib/events.js'
import { formatConversationTimestamp, formatMessageTimestamp } from '../lib/time.js'

type Phase = 'loading' | 'pairing' | 'ready'
type Theme = 'latte' | 'mocha'
type ComposerDraft = { text: string; files: File[] }

const initialState: LunaClientState = {
  conversations: [],
  messages: [],
  selectedConversationId: undefined,
  nextBeforeOrdinal: undefined,
  cursor: 0,
}

export function LunaApp() {
  const [phase, setPhase] = useState<Phase>('loading')
  const [client, setClient] = useState(initialState)
  const [error, setError] = useState<string>()
  const [search, setSearch] = useState('')
  const [theme, setTheme] = useState<Theme>('latte')
  const [drafts, setDrafts] = useState<Record<string, ComposerDraft>>({})
  const cursor = useRef(0)

  const installBootstrap = useCallback((bootstrap: Bootstrap) => {
    const conversations = sortConversations(bootstrap.conversations)
    cursor.current = bootstrap.cursor
    setClient({
      conversations,
      messages: [],
      selectedConversationId: conversations[0]?.id,
      nextBeforeOrdinal: undefined,
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
        if (event.type === 'sync.reset_required') {
          void api<Bootstrap>('/v1/bootstrap')
            .then(installBootstrap)
            .catch((requestError: unknown) => setError(messageFromError(requestError)))
          socket?.close()
          return
        }
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
  }, [installBootstrap, phase])

  useEffect(() => {
    const conversationId = client.selectedConversationId
    if (!conversationId || phase !== 'ready') return
    let active = true
    void api<ConversationMessages>(`/v1/conversations/${conversationId}/messages`)
      .then((response) => {
        if (active) {
          setClient((current) => ({
            ...current,
            messages: response.messages,
            nextBeforeOrdinal: response.nextBeforeOrdinal,
          }))
        }
      })
      .catch((requestError: unknown) => setError(messageFromError(requestError)))
    return () => {
      active = false
    }
  }, [client.selectedConversationId, phase])

  useEffect(() => {
    const conversationId = client.selectedConversationId
    if (!conversationId) return
    setDrafts((current) =>
      current[conversationId]
        ? current
        : {
            ...current,
            [conversationId]: { text: loadStoredDraft(conversationId), files: [] },
          },
    )
  }, [client.selectedConversationId])

  const updateDraft = (conversationId: string, update: (draft: ComposerDraft) => ComposerDraft) => {
    setDrafts((current) => {
      const next = update(
        current[conversationId] ?? { text: loadStoredDraft(conversationId), files: [] },
      )
      storeDraft(conversationId, next.text)
      return { ...current, [conversationId]: next }
    })
  }

  const clearDraft = (conversationId: string) => {
    storeDraft(conversationId, '')
    setDrafts((current) => ({
      ...current,
      [conversationId]: { text: '', files: [] },
    }))
  }

  const setSelectedConversation = (conversationId: string | undefined) => {
    setClient((current) => ({
      ...current,
      messages: [],
      selectedConversationId: conversationId,
      nextBeforeOrdinal: undefined,
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
        conversations: upsertConversation(current.conversations, conversation),
        selectedConversationId: conversation.id,
        messages: [],
        nextBeforeOrdinal: undefined,
      }))
    } catch (requestError) {
      setError(messageFromError(requestError))
    }
  }

  const loadEarlierMessages = async () => {
    const conversationId = client.selectedConversationId
    const before = client.nextBeforeOrdinal
    if (!conversationId || before === undefined) return
    try {
      const response = await api<ConversationMessages>(
        `/v1/conversations/${conversationId}/messages?beforeOrdinal=${String(before)}`,
      )
      setClient((current) => ({
        ...current,
        messages: mergeMessages(response.messages, current.messages),
        nextBeforeOrdinal: response.nextBeforeOrdinal,
      }))
    } catch (requestError) {
      setError(messageFromError(requestError))
    }
  }

  const removeArchivedConversation = (conversationId: string) => {
    storeDraft(conversationId, '')
    setDrafts((current) => {
      const next = { ...current }
      delete next[conversationId]
      return next
    })
    setClient((current) => ({
      ...current,
      conversations: current.conversations.filter(
        (conversation) => conversation.id !== conversationId,
      ),
      messages: [],
      selectedConversationId: undefined,
      nextBeforeOrdinal: undefined,
    }))
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
            aria-label="Search conversations"
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
            key={selected.id}
            conversation={selected}
            messages={client.messages}
            draft={drafts[selected.id] ?? { text: '', files: [] }}
            canLoadEarlier={client.nextBeforeOrdinal !== undefined}
            onLoadEarlier={() => void loadEarlierMessages()}
            onBack={() => setSelectedConversation(undefined)}
            onMessage={(message) =>
              setClient((current) => ({
                ...current,
                conversations: applyLatestMessage(current.conversations, message),
                messages:
                  current.selectedConversationId === message.conversationId
                    ? upsertMessage(current.messages, message)
                    : current.messages,
              }))
            }
            onDraftChange={(update) => updateDraft(selected.id, update)}
            onDraftSent={() => clearDraft(selected.id)}
            onArchived={() => removeArchivedConversation(selected.id)}
            onRename={(conversation) =>
              setClient((current) => ({
                ...current,
                conversations: upsertConversation(current.conversations, conversation),
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
  const [requestingCode, setRequestingCode] = useState(false)
  const [notice, setNotice] = useState<string>()
  const requestCode = async () => {
    setRequestingCode(true)
    setNotice(undefined)
    onError(undefined)
    try {
      const response = await api<PairingCodeRequestResponse>('/v1/pairing/request', {
        method: 'POST',
      })
      setCode('')
      setNotice(
        `A new code was written to Luna’s Citadel logs. It expires at ${new Date(
          response.expiresAt,
        ).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}.`,
      )
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setRequestingCode(false)
    }
  }
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
          Ask Luna for a one-time code, find the newest code in its Citadel logs, then enter it
          below. Your conversations stay on your Mac.
        </p>
        <button
          type="button"
          className="secondary-button pairing-request-button"
          disabled={requestingCode}
          onClick={() => void requestCode()}
        >
          {requestingCode ? 'Requesting…' : 'Ask for a pairing code'}
        </button>
        {notice && (
          <p className="form-notice" role="status">
            {notice}
          </p>
        )}
        <form onSubmit={(event) => void pair(event)}>
          <label>
            Pairing code
            <input
              autoFocus
              autoComplete="one-time-code"
              inputMode="numeric"
              pattern="[0-9]{6}"
              value={code}
              onChange={(event) =>
                setCode(event.target.value.replaceAll(/[^0-9]/g, '').slice(0, 6))
              }
              placeholder="123456"
              minLength={6}
              maxLength={6}
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
  const timestamp = conversation.lastMessageAt ?? conversation.createdAt
  return (
    <button className={`conversation-cell ${selected ? 'selected' : ''}`} onClick={onSelect}>
      <span className="avatar">
        {repo?.icon.contentUrl ? (
          <img src={repo.icon.contentUrl} alt="" />
        ) : (
          (repo?.icon.fallbackText ?? '☾')
        )}
      </span>
      <span className="cell-copy">
        <span className="cell-title">{conversation.title}</span>
        <span className="cell-preview">
          {conversation.preview || stateLabel(conversation.state)}
        </span>
      </span>
      <time className="cell-time" dateTime={timestamp} title={formatMessageTimestamp(timestamp)}>
        {formatConversationTimestamp(timestamp)}
      </time>
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
  draft,
  canLoadEarlier,
  onLoadEarlier,
  onBack,
  onMessage,
  onDraftChange,
  onDraftSent,
  onArchived,
  onRename,
  onError,
}: {
  conversation: Conversation
  messages: Message[]
  draft: ComposerDraft
  canLoadEarlier: boolean
  onLoadEarlier: () => void
  onBack: () => void
  onMessage: (message: Message) => void
  onDraftChange: (update: (draft: ComposerDraft) => ComposerDraft) => void
  onDraftSent: () => void
  onArchived: () => void
  onRename: (conversation: Conversation) => void
  onError: (message: string | undefined) => void
}) {
  const messageScroll = useRef<HTMLDivElement>(null)
  const positionedInitialMessages = useRef(false)
  const busy = ['working', 'compacting', 'retrying', 'restoring', 'starting'].includes(
    conversation.state,
  )
  useLayoutEffect(() => {
    const transcript = messageScroll.current
    if (!transcript || messages.length === 0) return
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    transcript.scrollTo({
      top: transcript.scrollHeight,
      behavior: positionedInitialMessages.current && !prefersReducedMotion ? 'smooth' : 'auto',
    })
    positionedInitialMessages.current = true
  }, [conversation.activities, conversation.taskList, messages])
  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onBack()
    }
    window.addEventListener('keydown', handleEscape)
    return () => window.removeEventListener('keydown', handleEscape)
  }, [onBack])

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

  const archive = async () => {
    if (!window.confirm(`Archive “${conversation.title}”?`)) return
    try {
      await api<void>(`/v1/conversations/${conversation.id}/archive`, { method: 'POST' })
      onArchived()
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
        <button
          className="icon-button"
          aria-label="Archive conversation"
          title="Archive conversation"
          onClick={() => void archive()}
        >
          <Archive size={17} />
        </button>
      </header>
      <div ref={messageScroll} className="message-scroll">
        {canLoadEarlier && (
          <button className="load-earlier" onClick={onLoadEarlier}>
            Load earlier messages
          </button>
        )}
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
        {conversation.taskList && (
          <TaskListProgress key={conversation.taskList.id} taskList={conversation.taskList} />
        )}
        {busy && <TypingIndicator activities={conversation.activities} />}
      </div>
      <Composer
        conversation={conversation}
        busy={busy}
        draft={draft}
        onDraftChange={onDraftChange}
        onDraftSent={onDraftSent}
        onMessage={onMessage}
        onError={onError}
      />
    </div>
  )
}

function MessageBubble({ message }: { message: Message }) {
  const [showsTimestamp, setShowsTimestamp] = useState(false)
  const descriptionId = `message-time-description-${message.id}`
  const toggleTimestamp = () => setShowsTimestamp((current) => !current)
  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    const target = event.target
    if (
      target instanceof Element &&
      target.closest('a, button, input, textarea, select, summary')
    ) {
      return
    }
    if (!window.getSelection()?.isCollapsed) return
    toggleTimestamp()
  }
  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.target !== event.currentTarget || !['Enter', ' '].includes(event.key)) return
    event.preventDefault()
    toggleTimestamp()
  }
  const formattedTimestamp = formatMessageTimestamp(message.createdAt)
  return (
    <article
      className={`message-row ${message.role}`}
      tabIndex={0}
      aria-describedby={descriptionId}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
    >
      <div className="message-stack">
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
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                {message.text}
              </ReactMarkdown>
              {message.status === 'streaming' && <span className="stream-caret" />}
            </div>
          ) : (
            <p>{message.text}</p>
          )}
        </div>
        {showsTimestamp && (
          <time className="message-timestamp" dateTime={message.createdAt}>
            {formattedTimestamp}
          </time>
        )}
        <span id={descriptionId} className="visually-hidden">
          {showsTimestamp
            ? `Sent ${formattedTimestamp}. Press Enter to hide the timestamp.`
            : 'Press Enter to show the sent date and time.'}
        </span>
      </div>
    </article>
  )
}

function Composer({
  conversation,
  busy,
  draft,
  onDraftChange,
  onDraftSent,
  onMessage,
  onError,
}: {
  conversation: Conversation
  busy: boolean
  draft: ComposerDraft
  onDraftChange: (update: (draft: ComposerDraft) => ComposerDraft) => void
  onDraftSent: () => void
  onMessage: (message: Message) => void
  onError: (message: string | undefined) => void
}) {
  const { text, files } = draft
  const [sending, setSending] = useState(false)
  const [recording, setRecording] = useState(false)
  const recorder = useRef<MediaRecorder | undefined>(undefined)
  const chunks = useRef<Blob[]>([])
  const fileInput = useRef<HTMLInputElement>(null)
  const cameraInput = useRef<HTMLInputElement>(null)
  const textarea = useRef<HTMLTextAreaElement>(null)

  useEffect(() => resizeTextarea(textarea.current), [text])
  useEffect(() => {
    const handleResize = () => resizeTextarea(textarea.current)
    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
  }, [])

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
    onDraftChange((current) => ({
      ...current,
      files: [...current.files, ...incoming.filter((file) => file.type.startsWith('image/'))].slice(
        0,
        6,
      ),
    }))
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
      onDraftSent()
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
        const mimeType = next.mimeType || 'audio/webm'
        const extension = mimeType.includes('mp4')
          ? 'm4a'
          : mimeType.includes('ogg')
            ? 'ogg'
            : 'webm'
        const blob = new Blob(chunks.current, { type: mimeType })
        const body = new FormData()
        body.append('file', blob, `recording.${extension}`)
        void api<TranscriptionResponse>('/v1/transcriptions', { method: 'POST', body })
          .then((response) =>
            onDraftChange((current) => ({
              ...current,
              text: `${current.text}${current.text ? ' ' : ''}${response.text}`,
            })),
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
                aria-label={`Remove ${file.name}`}
                onClick={() =>
                  onDraftChange((current) => ({
                    ...current,
                    files: current.files.filter((item) => item !== file),
                  }))
                }
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
          ref={textarea}
          value={text}
          rows={1}
          aria-label={busy ? 'Steer Pi' : 'Message Luna'}
          placeholder={busy ? 'Steer Pi…' : 'Message Luna…'}
          onChange={(event) => {
            onDraftChange((current) => ({ ...current, text: event.target.value }))
            resizeTextarea(event.currentTarget)
          }}
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
          accept="image/png,image/jpeg,image/gif,image/webp,image/heic,image/heif,.heic,.heif"
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

function loadStoredDraft(conversationId: string): string {
  try {
    return window.localStorage.getItem(`luna-draft:${conversationId}`) ?? ''
  } catch {
    return ''
  }
}

function storeDraft(conversationId: string, text: string) {
  try {
    if (text) window.localStorage.setItem(`luna-draft:${conversationId}`, text)
    else window.localStorage.removeItem(`luna-draft:${conversationId}`)
  } catch {
    // The in-memory draft remains available when browser storage is unavailable or full.
  }
}

function resizeTextarea(textarea: HTMLTextAreaElement | null) {
  if (!textarea) return
  textarea.style.height = '0px'
  const maximumHeight = Number.parseFloat(window.getComputedStyle(textarea).maxHeight)
  const contentHeight = textarea.scrollHeight
  textarea.style.height = `${String(Math.min(contentHeight, maximumHeight))}px`
  textarea.style.overflowY = contentHeight > maximumHeight ? 'auto' : 'hidden'
}

function TaskListProgress({ taskList }: { taskList: AgentTaskList }) {
  const completed = taskList.tasks.filter((task) => task.status === 'completed').length
  const skipped = taskList.tasks.filter((task) => task.status === 'skipped').length
  const resolved = completed + skipped
  const current =
    taskList.tasks.find((task) => task.status === 'in_progress') ??
    taskList.tasks.find((task) => task.status === 'blocked') ??
    taskList.tasks.find((task) => task.status === 'pending')
  const finished = resolved === taskList.tasks.length

  return (
    <details className={`task-progress ${finished ? 'complete' : ''}`}>
      <summary>
        <span className="task-progress-mark" aria-hidden="true">
          <ListChecks size={17} />
        </span>
        <span className="task-progress-copy">
          <strong>{taskList.title ?? 'Plan progress'}</strong>
          <span>
            {finished
              ? skipped
                ? 'Plan finished'
                : 'Plan complete'
              : (current?.text ?? 'Reviewing remaining work')}
          </span>
        </span>
        <span className="task-progress-count">
          {resolved}/{taskList.tasks.length}
        </span>
        <ChevronDown className="task-progress-chevron" size={15} aria-hidden="true" />
      </summary>
      <progress
        value={resolved}
        max={taskList.tasks.length}
        aria-label={`${String(completed)} of ${String(taskList.tasks.length)} tasks completed${skipped ? `, ${String(skipped)} skipped` : ''}`}
      />
      <ol>
        {taskList.tasks.map((task) => (
          <li key={task.id} className={task.status}>
            <span className="task-status-mark" aria-hidden="true">
              {task.status === 'completed' ? (
                <Check size={14} />
              ) : task.status === 'blocked' ? (
                <TriangleAlert size={14} />
              ) : task.status === 'skipped' ? (
                <Minus size={14} />
              ) : (
                <Circle size={12} />
              )}
            </span>
            <span>
              <strong>{task.text}</strong>
              {task.note && <small>{task.note}</small>}
            </span>
          </li>
        ))}
      </ol>
    </details>
  )
}

function TypingIndicator({ activities }: { activities: AgentActivity[] }) {
  const latest = activities.at(-1)
  return (
    <div className="message-row assistant">
      <div className="progress-indicator" aria-label="Pi is working">
        <span className="typing-dots" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        {latest &&
          (activities.length > 1 ? (
            <details className="activity-details">
              <summary>
                <span>{latest.summary}</span>
                <ChevronDown size={14} aria-hidden="true" />
              </summary>
              <ol>
                {activities.map((activity) => (
                  <li key={activity.id}>{activity.summary}</li>
                ))}
              </ol>
            </details>
          ) : (
            <span className="activity-latest">{latest.summary}</span>
          ))}
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

function mergeMessages(earlier: Message[], current: Message[]): Message[] {
  const messages = new Map<string, Message>()
  for (const message of [...earlier, ...current]) messages.set(message.id, message)
  return [...messages.values()].sort((left, right) => left.ordinal - right.ordinal)
}

function upsertMessage(messages: Message[], message: Message): Message[] {
  const found = messages.some((item) => item.id === message.id)
  return found
    ? messages.map((item) => (item.id === message.id ? message : item))
    : [...messages, message]
}
