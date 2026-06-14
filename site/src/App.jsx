import reactLogo from './assets/react.svg'
import viteLogo from './assets/vite.svg'
import heroImg from './assets/hero.png'
import './App.css'

function App() {
  return (
    <div className="app-shell">
      <div className="ambient" aria-hidden="true">
        <span className="orb orb-a"></span>
        <span className="orb orb-b"></span>
        <span className="orb orb-c"></span>
      </div>

      <header className="topbar">
        <p className="event-pill">100% Local AI — No Cloud. No API. No Limits.</p>
        <div className="stack">
          <span>
            <img src={reactLogo} alt="React" /> Runs On-Device
          </span>
          <span>
            <img src={viteLogo} alt="Vite" /> Zero Data Leakage
          </span>
        </div>
      </header>

      <main>
        <section className="hero-section">
          <div className="hero-copy">
            <p className="kicker">Fully Local. Blazing Fast. Genuinely Private.</p>
            <h1>Hey Dot</h1>
            <p className="subtitle">
              A desktop AI that runs entirely on your machine. No Gemini. No
              OpenAI. No internet required. Just say "Hey Dot" and get instant
              screen-aware intelligence powered by a local model.
            </p>
            <div className="hero-meta">
              <div>
                <span className="meta-label">AI Engine</span>
                <strong>Local Model via Ollama</strong>
              </div>
              <div>
                <span className="meta-label">Cloud Dependency</span>
                <strong>None. Zero. Nada.</strong>
              </div>
            </div>
            <div className="hero-actions">
              <a href="#highlights">Why Local Wins</a>
              <a href="#workflow" className="ghost">
                How It Works
              </a>
            </div>
          </div>

          <div className="hero-visual" aria-hidden="true">
            <img src={heroImg} className="base" width="170" height="179" alt="" />
            <div className="pulse-ring pulse-a"></div>
            <div className="pulse-ring pulse-b"></div>
            <div className="badge">Your AI. Your Machine. Your Rules.</div>
          </div>
        </section>

        <section id="highlights" className="panel-grid">
          <article>
            <h2>Offline-First Intelligence</h2>
            <p>
              The entire AI brain runs locally via Ollama. No API keys, no
              subscriptions, no latency from a distant data center.
            </p>
          </article>
          <article>
            <h2>Screen Context, Locally Processed</h2>
            <p>
              Your screen is captured and analyzed on-device. Nothing leaves
              your machine — not your screen, not your voice, not your queries.
            </p>
          </article>
          <article>
            <h2>Instant Voice Loop</h2>
            <p>
              Wake word → question → local inference → spoken answer. The full
              loop happens right here, without touching the cloud.
            </p>
          </article>
        </section>

        <section id="workflow" className="team-panel">
          <div>
            <p className="kicker">Built At HackBLR — Entirely On Local AI</p>
            <h2>How We Built Hey Dot At Hackathon Speed</h2>
            <p>
              We replaced every cloud dependency with a local vision-language
              model. Wake word, screen capture, inference, and voice output —
              all on your machine, all in seconds.
            </p>
          </div>
          <ul>
            <li>
              <span>Step 1</span>
              <strong>Detect "Hey Dot" Wake Word</strong>
            </li>
            <li>
              <span>Step 2</span>
              <strong>Capture Voice Question On-Device</strong>
            </li>
            <li>
              <span>Step 3</span>
              <strong>Run Local Vision-Language Model</strong>
            </li>
            <li>
              <span>Step 4</span>
              <strong>Speak The Answer. Privately.</strong>
            </li>
          </ul>
        </section>

        <section className="footer-note">
          <p>
            Hey Dot: the only desktop AI assistant that keeps everything —
            your screen, your voice, your data — entirely on your machine.
          </p>
        </section>
      </main>
    </div>
  )
}

export default App
