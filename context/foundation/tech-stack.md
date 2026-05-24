---
starter_id: axum
package_manager: cargo
project_name: rc-smart-pit-stop
hints:
  language_family: rust
  team_size: solo
  deployment_target: self-host
  ci_provider: github-actions
  ci_default_flow: auto-deploy-on-merge
  bootstrapper_confidence: first-class
  path_taken: custom
  quality_override: false
  self_check_answers:
    typed: true
    from_official_starter: true
    conventions: true
    docs_current: true
    can_judge_agent: true
  has_auth: true
  has_payments: false
  has_realtime: false
  has_ai: false
  has_background_jobs: true
---

## Why this stack

RC Smart Pit-Stop is a small, after-hours MVP with login, simulated telemetry, and background job needs, and you chose a Rust backend plus TypeScript frontend architecture. The registry has no single full-stack Rust+TypeScript dashboard starter, so Axum is the backend starter: type-safe, mainstream within Rust web services, documented, and suitable for an authenticated API. The frontend should be added as a Vite + React + TypeScript dashboard alongside Axum. The system also needs an MQTT telemetry simulator and broker; for the MVP, Axum should own the telemetry consumer and internal scheduled state checks, with a separate worker service left as a later split if load or complexity grows. Self-host keeps Docker Compose practical for the API, frontend, database, broker, and simulator shape. GitHub Actions with auto-deploy-on-merge gives the simplest solo CI/CD path. Bootstrapper confidence is first-class, so scaffolding should mostly work but may need a few manual adjustments.
