import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = { children: ReactNode; compact?: boolean };
type State = { error: Error | null };

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Grafiki desktop render failed", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    if (this.props.compact) {
      // Pane-scoped fallback: the rail and any live terminal stay usable;
      // only the crashed view is replaced.
      return (
        <div className="pane-error" role="alert">
          <p>
            <b>This view hit an unexpected error.</b>{" "}
            {this.state.error.message || "It could not be rendered."}
          </p>
          <div className="pane-error-actions">
            <button type="button" onClick={() => this.setState({ error: null })}>
              Try again
            </button>
            <button type="button" onClick={() => window.location.reload()}>
              Reload Grafiki
            </button>
          </div>
        </div>
      );
    }
    return (
      <main className="fatal-error" role="alert">
        <span className="brand-mark">G</span>
        <h1>Grafiki hit an unexpected UI error</h1>
        <p>{this.state.error.message || "The desktop interface could not continue."}</p>
        <button type="button" onClick={() => window.location.reload()}>
          Reload Grafiki
        </button>
      </main>
    );
  }
}
