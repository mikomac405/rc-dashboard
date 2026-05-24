import { useEffect, useMemo, useState } from 'react'
import './App.css'

type TelemetryReading = {
  car_id: string
  lap: number
  tire_wear: number
  battery_temp_c: number
  motor_temp_c: number
  speed_kph: number
  pit_recommended: boolean
  timestamp_ms: number
}

function App() {
  const [readings, setReadings] = useState<TelemetryReading[]>([])
  const [status, setStatus] = useState<'loading' | 'live' | 'offline'>('loading')

  useEffect(() => {
    let active = true

    async function loadTelemetry() {
      try {
        const response = await fetch('/api/telemetry')
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
  }, [])

  const latest = readings[0]
  const pitCount = useMemo(
    () => readings.filter((reading) => reading.pit_recommended).length,
    [readings],
  )

  return (
    <main className="dashboard">
      <header className="topbar">
        <div>
          <p className="eyebrow">RC Smart Pit-Stop</p>
          <h1>Race Control</h1>
        </div>
        <span className={`status ${status}`}>{status}</span>
      </header>

      <section className="summary-grid">
        <Metric label="Current car" value={latest?.car_id ?? 'waiting'} />
        <Metric label="Lap" value={latest?.lap.toString() ?? '-'} />
        <Metric
          label="Speed"
          value={latest ? `${latest.speed_kph.toFixed(1)} kph` : '-'}
        />
        <Metric label="Pit alerts" value={pitCount.toString()} tone="alert" />
      </section>

      <section className="telemetry-panel">
        <div className="panel-heading">
          <h2>Telemetry Stream</h2>
          <span>{readings.length} samples</span>
        </div>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Car</th>
                <th>Lap</th>
                <th>Tire wear</th>
                <th>Battery</th>
                <th>Motor</th>
                <th>Speed</th>
                <th>Call</th>
              </tr>
            </thead>
            <tbody>
              {readings.length === 0 ? (
                <tr>
                  <td colSpan={7}>Waiting for simulator telemetry</td>
                </tr>
              ) : (
                readings.slice(0, 12).map((reading) => (
                  <tr key={`${reading.car_id}-${reading.timestamp_ms}`}>
                    <td>{reading.car_id}</td>
                    <td>{reading.lap}</td>
                    <td>{reading.tire_wear.toFixed(1)}%</td>
                    <td>{reading.battery_temp_c.toFixed(1)}C</td>
                    <td>{reading.motor_temp_c.toFixed(1)}C</td>
                    <td>{reading.speed_kph.toFixed(1)}</td>
                    <td>
                      <span
                        className={
                          reading.pit_recommended ? 'call pit' : 'call stay'
                        }
                      >
                        {reading.pit_recommended ? 'Pit' : 'Stay out'}
                      </span>
                    </td>
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

function Metric({
  label,
  value,
  tone,
}: {
  label: string
  value: string
  tone?: 'alert'
}) {
  return (
    <article className={`metric ${tone ?? ''}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  )
}

export default App
