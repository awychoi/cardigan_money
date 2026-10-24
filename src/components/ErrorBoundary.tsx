import { Component, type ErrorInfo, type ReactNode } from "react";
import { logEvent } from "../lib/logging";

interface Props {
  children: ReactNode;
}
interface State {
  failed: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Fixed metadata only; never the message or stack (they may contain data).
    logEvent("ui_error", { phase: "render", ok: false });
    void error;
    void info;
  }

  render(): ReactNode {
    if (this.state.failed) {
      return (
        <main className="fatal-screen" role="alert">
          <h1>cardigan_money</h1>
          <p>A display error occurred.</p>
          <button type="button" onClick={() => this.setState({ failed: false })}>
            Try again
          </button>
          <p className="fine-print">No ledger data is shown while the view is in an error state.</p>
        </main>
      );
    }
    return this.props.children;
  }
}
