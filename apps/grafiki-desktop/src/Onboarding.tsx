import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import { initializeProject, listLocalModels, pickProjectFolder, updateCaptureConfig } from "./api";

/// First-run onboarding (UX_REDESIGN.md §5.0): four steps, under 90 seconds,
/// honest at every fork. Full-sheet, no rail — nothing else exists until the
/// user is inside their first captured session.
export default function Onboarding(props: {
  onProjectReady: (projectDir: string) => void;
  onFinished: (launch: string | null) => void;
}) {
  const [step, setStep] = useState(0);
  const [folder, setFolder] = useState("");
  const [initializing, setInitializing] = useState(false);
  const [captureConsent, setCaptureConsent] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);
  const [models, setModels] = useState<string[] | null>(null);

  useEffect(() => {
    if (step === 2 && models === null) {
      listLocalModels()
        .then(setModels)
        .catch(() => setModels([]));
    }
  }, [step, models]);

  const browse = async () => {
    const selected = await pickProjectFolder(folder || undefined);
    if (selected) setFolder(selected);
  };

  const initialize = async () => {
    const dir = folder.trim();
    if (!dir) return;
    setInitializing(true);
    setInitError(null);
    try {
      await initializeProject({ projectDir: dir });
      if (captureConsent) {
        // The default capture config ships terminal output OFF; this is the
        // explicit consent moment that turns it on for this workspace.
        await updateCaptureConfig({ startDir: dir, terminal: true, terminalOutput: "full" });
      }
      props.onProjectReady(dir);
      setStep(2);
    } catch (error) {
      setInitError(String(error));
    } finally {
      setInitializing(false);
    }
  };

  const [enablingCapture, setEnablingCapture] = useState(false);
  const enableCaptureLate = async () => {
    const dir = folder.trim();
    if (!dir) return;
    setEnablingCapture(true);
    try {
      await updateCaptureConfig({ startDir: dir, terminal: true, terminalOutput: "full" });
      setCaptureConsent(true);
    } catch (error) {
      setInitError(String(error));
    } finally {
      setEnablingCapture(false);
    }
  };

  const agents = [
    { label: "Claude Code", cmd: "claude" },
    { label: "Codex", cmd: "codex" },
    { label: "Gemini", cmd: "gemini" },
    { label: "Shell", cmd: "" },
  ];

  return (
    <div className="onboarding">
      <motion.div
        key={step}
        className="onboarding-step"
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.18, ease: [0.2, 0, 0, 1] }}
      >
        {step === 0 ? (
          <>
            <span className="brand-mark onboarding-mark">G</span>
            <h1>
              Your AI forgets every session.
              <br />
              Grafiki <em>remembers</em>.
            </h1>
            <p className="muted">
              Grafiki listens to your coding sessions, keeps the decisions and gotchas as
              reviewable memory, and briefs your agent next time. Local-first — nothing leaves
              this Mac.
            </p>
            <button className="button primary" onClick={() => setStep(1)}>
              Get started
            </button>
          </>
        ) : null}

        {step === 1 ? (
          <>
            <h1>Where do you work?</h1>
            <p className="muted">
              Pick the project folder Grafiki should remember. It creates a private memory
              database for it (you can add more projects later in Settings).
            </p>
            <div className="onboarding-folder-field">
              <label htmlFor="onboarding-project-folder">
                Project folder
              </label>
              <div className="onboarding-folder">
                <input
                  id="onboarding-project-folder"
                  value={folder}
                  onChange={(event) => setFolder(event.target.value)}
                  placeholder="/path/to/your/project"
                />
                <button className="button" onClick={() => void browse()}>
                  Browse…
                </button>
              </div>
            </div>
            {initError ? <p role="alert" style={{ color: "var(--danger)" }}>{initError}</p> : null}
            <label className="onboarding-consent">
              <input
                type="checkbox"
                checked={captureConsent}
                onChange={(event) => setCaptureConsent(event.currentTarget.checked)}
              />
              <span>
                I consent to storing full terminal output from this workspace as local capture
                events so it can become reviewable memory. This is off by default, stays on this
                Mac, and can be changed anytime in Settings &rarr; Capture &amp; privacy.
              </span>
            </label>
            <button
              className="button primary"
              disabled={!folder.trim() || initializing}
              onClick={() => void initialize()}
            >
              {initializing ? "Initializing…" : "Create memory here"}
            </button>
          </>
        ) : null}

        {step === 2 ? (
          <>
            <h1>Local AI</h1>
            {models === null ? (
              <p className="muted">Checking for Ollama…</p>
            ) : models.length > 0 ? (
              <>
                <p className="muted">
                  Found Ollama with {models.length} model{models.length === 1 ? "" : "s"} —
                  Grafiki will use <strong>{models[0]}</strong> to turn sessions into memory,
                  entirely on this machine.
                </p>
                <button className="button primary" onClick={() => setStep(3)}>
                  Continue
                </button>
              </>
            ) : (
              <>
                <p className="muted">
                  Ollama isn't reachable. Grafiki still records your sessions now; automatic
                  memory extraction starts as soon as a local model exists. Install{" "}
                  <a href="https://ollama.com" target="_blank" rel="noreferrer">
                    Ollama
                  </a>{" "}
                  and run <code>ollama pull gemma3:1b</code> whenever you like.
                </p>
                <button className="button primary" onClick={() => setStep(3)}>
                  Skip for now
                </button>
              </>
            )}
          </>
        ) : null}

        {step === 3 ? (
          <>
            <h1>Start your first session</h1>
            <p className="muted">
              {captureConsent
                ? "Terminal capture is on for this workspace. What you decide and learn can show up on Home for review."
                : "Terminal capture is off. Your session still runs normally, and no terminal output will be stored unless you enable capture."}
            </p>
            {!captureConsent ? (
              // Last chance to fix the day-one failure mode: starting the first
              // session with capture off means Grafiki remembers nothing and the
              // product promise silently fails. One click, still explicit consent.
              <button
                className="button"
                disabled={enablingCapture}
                onClick={() => void enableCaptureLate()}
              >
                {enablingCapture ? "Turning on capture…" : "Turn on capture for this workspace"}
              </button>
            ) : null}
            <div className="agent-buttons">
              {agents.map((agent) => (
                <button
                  key={agent.label}
                  className="button"
                  onClick={() => props.onFinished(agent.cmd)}
                >
                  {agent.label}
                </button>
              ))}
            </div>
            <button className="link-button" onClick={() => props.onFinished(null)}>
              Skip — take me to the app
            </button>
          </>
        ) : null}
      </motion.div>
      <div className="onboarding-dots">
        {[0, 1, 2, 3].map((dot) => (
          <span key={dot} className={dot === step ? "active" : ""} />
        ))}
      </div>
    </div>
  );
}
