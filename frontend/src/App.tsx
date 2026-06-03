import { useEffect, useMemo, useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import './App.css'

const TOKEN_STORAGE_KEY = 'rc-smart-pit-stop-token'

type TelemetryReading = {
  vehicle_id: string
  health_state: 'healthy' | 'unhealthy' | 'dead'
  health_reason:
    | 'none'
    | 'low_battery'
    | 'overheated_motor'
    | 'speed_mismatch'
    | 'jammed'
    | 'lost_connection'
  battery_percent: number
  heartbeat_age_ms: number
  motor_temp_c: number
  commanded_speed_kph: number
  actual_speed_kph: number
  jam_detected: boolean
  mission_area: string
  manual_pickup_required: boolean
  timestamp_ms: number
}

type Session = {
  token: string
  expires_at_ms: number
}

type LoginResponse = Session & {
  must_change_password: boolean
}

type AuthView = 'login' | 'password-change' | 'authenticated'

type ApiError = {
  status: number
  message: string
}

function App() {
  const [authView, setAuthView] = useState<AuthView>(() =>
    readStoredSession() ? 'authenticated' : 'login',
  )
  const [session, setSession] = useState<Session | null>(() =>
    readStoredSession(),
  )
  const [passwordChangeToken, setPasswordChangeToken] = useState<string | null>(
    null,
  )
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [authMessage, setAuthMessage] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [readings, setReadings] = useState<TelemetryReading[]>([])
  const [status, setStatus] = useState<'loading' | 'live' | 'offline'>(
    session ? 'loading' : 'offline',
  )

  function resetToLogin(message = '') {
    clearStoredSession()
    setSession(null)
    setPasswordChangeToken(null)
    setAuthView('login')
    setPassword('')
    setNewPassword('')
    setReadings([])
    setStatus('offline')
    setAuthMessage(message)
  }

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setIsSubmitting(true)
    setAuthMessage('')

    try {
      const response = await postJson<LoginResponse>('/api/auth/login', {
        username,
        password,
      })

      if (response.must_change_password) {
        clearStoredSession()
        setPasswordChangeToken(response.token)
        setAuthView('password-change')
        setPassword('')
        setAuthMessage('Choose a new password before opening telemetry.')
        return
      }

      const nextSession = {
        token: response.token,
        expires_at_ms: response.expires_at_ms,
      }
      storeSession(nextSession)
      setSession(nextSession)
      setPasswordChangeToken(null)
      setAuthView('authenticated')
      setPassword('')
      setStatus('loading')
    } catch (error) {
      setAuthMessage(authErrorMessage(error, 'Login failed.'))
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handlePasswordChange(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()

    if (!passwordChangeToken) {
      resetToLogin('Password change session expired. Sign in again.')
      return
    }

    setIsSubmitting(true)
    setAuthMessage('')

    try {
      const response = await postJson<LoginResponse>(
        '/api/auth/change-password',
        { new_password: newPassword },
        passwordChangeToken,
      )
      const nextSession = {
        token: response.token,
        expires_at_ms: response.expires_at_ms,
      }
      storeSession(nextSession)
      setSession(nextSession)
      setPasswordChangeToken(null)
      setAuthView('authenticated')
      setNewPassword('')
      setStatus('loading')
    } catch (error) {
      setAuthMessage(authErrorMessage(error, 'Password change failed.'))
    } finally {
      setIsSubmitting(false)
    }
  }

  useEffect(() => {
    if (authView !== 'authenticated' || !session) {
      return
    }

    let active = true
    const activeSession = session

    async function loadTelemetry() {
      try {
        const response = await fetch('/api/telemetry', {
          headers: {
            Authorization: `Bearer ${activeSession.token}`,
          },
        })

        if (response.status === 401) {
          if (active) {
            resetToLogin('Session expired. Sign in again.')
          }
          return
        }

        if (!response.ok) {
          throw new Error(`API responded with ${response.status}`)
        }

        const data = (await response.json()) as TelemetryReading[]
        if (active) {
          setReadings(data)
          setStatus('live')
        }
      } catch {
        if (active) {
          setStatus('offline')
        }
      }
    }

    loadTelemetry()
    const interval = window.setInterval(loadTelemetry, 2500)

    return () => {
      active = false
      window.clearInterval(interval)
    }
  }, [authView, session])

  const latest = readings[0]
  const manualPickupCount = useMemo(
    () => readings.filter((reading) => reading.manual_pickup_required).length,
    [readings],
  )
  const deadCount = useMemo(
    () => readings.filter((reading) => reading.health_state === 'dead').length,
    [readings],
  )

  if (authView === 'login') {
    return (
      <AuthShell>
        <form className="auth-panel" onSubmit={handleLogin}>
          <div className="auth-heading">
            <p className="eyebrow">RC Smart Pit-Stop</p>
            <h1>Fleet manager login</h1>
          </div>
          <label>
            <span>Username</span>
            <input
              autoComplete="username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              required
            />
          </label>
          <label>
            <span>Password</span>
            <input
              autoComplete="current-password"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
            />
          </label>
          {authMessage ? <p className="auth-message">{authMessage}</p> : null}
          <button type="submit" disabled={isSubmitting}>
            {isSubmitting ? 'Signing in' : 'Sign in'}
          </button>
        </form>
      </AuthShell>
    )
  }

  if (authView === 'password-change') {
    return (
      <AuthShell>
        <form className="auth-panel" onSubmit={handlePasswordChange}>
          <div className="auth-heading">
            <p className="eyebrow">RC Smart Pit-Stop</p>
            <h1>Change password</h1>
          </div>
          <label>
            <span>New password</span>
            <input
              autoComplete="new-password"
              minLength={1}
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
              required
            />
          </label>
          {authMessage ? <p className="auth-message">{authMessage}</p> : null}
          <div className="auth-actions">
            <button type="submit" disabled={isSubmitting}>
              {isSubmitting ? 'Updating' : 'Update password'}
            </button>
            <button className="secondary" type="button" onClick={() => resetToLogin()}>
              Back
            </button>
          </div>
        </form>
      </AuthShell>
    )
  }

  return (
    <main className="dashboard">
      <header className="topbar">
        <div>
          <p className="eyebrow">RC Scout Dashboard</p>
          <h1>Health Telemetry</h1>
        </div>
        <div className="session-tools">
          <span className={`status ${status}`}>{status}</span>
          <button className="secondary" type="button" onClick={() => resetToLogin()}>
            Log out
          </button>
        </div>
      </header>

      <section className="summary-grid">
        <Metric label="Current vehicle" value={latest?.vehicle_id ?? 'waiting'} />
        <Metric
          label="Health state"
          value={latest ? formatLabel(latest.health_state) : '-'}
          tone={healthMetricTone(latest)}
        />
        <Metric
          label="Manual pickups"
          value={manualPickupCount.toString()}
          tone={manualPickupCount > 0 ? 'critical' : undefined}
        />
        <Metric
          label="Mission area"
          value={latest?.mission_area ?? '-'}
          meta={deadCount > 0 ? `${deadCount} dead` : undefined}
        />
      </section>

      <section className="telemetry-panel">
        <div className="panel-heading">
          <h2>Health Stream</h2>
          <span>{readings.length} samples</span>
        </div>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Vehicle</th>
                <th>Area</th>
                <th>State</th>
                <th>Reason</th>
                <th>Battery</th>
                <th>Motor</th>
                <th>Speed</th>
                <th>Heartbeat</th>
                <th>Jam</th>
                <th>Pickup</th>
                <th>Sample</th>
              </tr>
            </thead>
            <tbody>
              {readings.length === 0 ? (
                <tr>
                  <td colSpan={11}>Waiting for simulator telemetry</td>
                </tr>
              ) : (
                readings.slice(0, 12).map((reading) => (
                  <tr key={`${reading.vehicle_id}-${reading.timestamp_ms}`}>
                    <td>{reading.vehicle_id}</td>
                    <td>{reading.mission_area}</td>
                    <td>
                      <span className={`state-badge ${reading.health_state}`}>
                        {formatLabel(reading.health_state)}
                      </span>
                    </td>
                    <td>{formatLabel(reading.health_reason)}</td>
                    <td>{reading.battery_percent.toFixed(1)}%</td>
                    <td>{reading.motor_temp_c.toFixed(1)}C</td>
                    <td>
                      <span className="speed-pair">
                        {reading.commanded_speed_kph.toFixed(1)} cmd
                        <span>{reading.actual_speed_kph.toFixed(1)} actual</span>
                      </span>
                    </td>
                    <td>{formatDuration(reading.heartbeat_age_ms)}</td>
                    <td>
                      <span className={`flag ${reading.jam_detected ? 'jammed' : ''}`}>
                        {reading.jam_detected ? 'Jammed' : 'Clear'}
                      </span>
                    </td>
                    <td>
                      <span
                        className={`flag ${reading.manual_pickup_required ? 'pickup' : ''}`}
                      >
                        {reading.manual_pickup_required ? 'Required' : 'No'}
                      </span>
                    </td>
                    <td>{formatSampleTime(reading.timestamp_ms)}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </section>
    </main>
  )
}

function AuthShell({ children }: { children: ReactNode }) {
  return <main className="auth-page">{children}</main>
}

function Metric({
  label,
  value,
  tone,
  meta,
}: {
  label: string
  value: string
  tone?: 'alert' | 'critical'
  meta?: string
}) {
  return (
    <article className={`metric ${tone ?? ''}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      {meta ? <small>{meta}</small> : null}
    </article>
  )
}

function formatLabel(value: string) {
  return value
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ')
}

function healthMetricTone(reading?: TelemetryReading) {
  if (reading?.health_state === 'dead') {
    return 'critical'
  }

  if (reading?.health_state === 'unhealthy') {
    return 'alert'
  }

  return undefined
}

function formatDuration(milliseconds: number) {
  if (milliseconds >= 1000) {
    return `${(milliseconds / 1000).toFixed(1)}s`
  }

  return `${milliseconds.toFixed(0)}ms`
}

function formatSampleTime(timestampMs: number) {
  return new Date(timestampMs).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

async function postJson<T>(url: string, body: unknown, token?: string): Promise<T> {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  })

  if (!response.ok) {
    throw {
      status: response.status,
      message: `API responded with ${response.status}`,
    } satisfies ApiError
  }

  return (await response.json()) as T
}

function readStoredSession(): Session | null {
  const rawSession = window.localStorage.getItem(TOKEN_STORAGE_KEY)
  if (!rawSession) {
    return null
  }

  try {
    const session = JSON.parse(rawSession) as Session
    if (!session.token || session.expires_at_ms <= Date.now()) {
      clearStoredSession()
      return null
    }

    return session
  } catch {
    clearStoredSession()
    return null
  }
}

function storeSession(session: Session) {
  window.localStorage.setItem(TOKEN_STORAGE_KEY, JSON.stringify(session))
}

function clearStoredSession() {
  window.localStorage.removeItem(TOKEN_STORAGE_KEY)
}

function authErrorMessage(error: unknown, fallback: string) {
  if (isApiError(error)) {
    if (error.status === 401) {
      return 'Invalid credentials or expired session.'
    }

    if (error.status === 503) {
      return 'Authentication is unavailable while the database is offline.'
    }
  }

  return fallback
}

function isApiError(error: unknown): error is ApiError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'status' in error &&
    typeof (error as ApiError).status === 'number'
  )
}

export default App
