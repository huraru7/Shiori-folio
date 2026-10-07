import { Component, type ErrorInfo, type ReactNode } from "react";
import { api } from "../../api/tauri";
import "./ErrorBoundary.css";

interface Props {
  // どの画面のエラーかを、表示とログで分かるようにする。
  label: string;
  children: ReactNode;
}

interface State {
  error: Error | null;
}

// 画面ごとのエラーの境界(詩織Ver4.0)。それまでは、どこか1つの画面が描画中に
// 失敗すると、アプリ全体が真っ白になっていた。ここで受け止めて、その画面だけを
// 「再読み込み」で作り直せるようにする。エラーはログ(data/logs/)にも残す。
class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    api
      .logFrontendError(`[${this.props.label}] ${error.message}\n${info.componentStack ?? ""}`)
      .catch(() => undefined);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="error-boundary" role="alert">
        <p className="error-boundary__title">「{this.props.label}」の画面を表示できませんでした。</p>
        <p className="error-boundary__message">{this.state.error.message}</p>
        <button type="button" className="error-boundary__retry" onClick={() => this.setState({ error: null })}>
          再読み込み
        </button>
      </div>
    );
  }
}

export default ErrorBoundary;
