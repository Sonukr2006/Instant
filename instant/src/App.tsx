import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { clearAuthSession, getAuthSession, saveAuthSession } from "./services/authService";
import { fetchAIResponse } from "./services/aiService";
import "./App.css";

const ResponseArea = lazy(() => import("./components/ResponseArea"));

type CapturedContextEvent = {
  text?: string | null;
  error?: string | null;
  source: "selected_text" | "clipboard";
};

function formatError(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return fallback;
}

function reportClientError(message: string) {
  if (import.meta.env.DEV) {
    console.error(message);
  }
}

function App() {
  const [chatPrompt, setChatPrompt] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const [loading, setLoading] = useState(false);
  const [aiResponse, setAiResponse] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<"chat" | "response">("chat");
  const [authTokenInput, setAuthTokenInput] = useState("");
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [authPanelOpen, setAuthPanelOpen] = useState(false);
  const userEditedPromptRef = useRef(false);
  const aiRequestIdRef = useRef(0);
  const aiLoadingRef = useRef(false);

  const fetchClipboardText = useCallback(async (options?: { force?: boolean }) => {
    const shouldOverwritePrompt = options?.force || !userEditedPromptRef.current;

    try {
      setLoading(true);
      setErrorMessage("");

      const text = await invoke<string>("get_clipboard_text");

      if (shouldOverwritePrompt) {
        setChatPrompt(text);
        userEditedPromptRef.current = false;
      }
    } catch (error) {
      const message = formatError(error, "Unable to read clipboard text.");

      reportClientError("Failed to fetch clipboard text.");
      if (shouldOverwritePrompt) {
        setChatPrompt("");
      }
      setErrorMessage(message);
    } finally {
      setLoading(false);
    }
  }, []);

  async function handleAskAI() {
    const requestId = aiRequestIdRef.current + 1;
    aiRequestIdRef.current = requestId;

    try {
      setAiLoading(true);
      setAiResponse("");
      setActiveTab("response");

      const response = await fetchAIResponse(chatPrompt);

      if (aiRequestIdRef.current === requestId) {
        setAiResponse(response);
      }
    } catch (error) {
      const message = formatError(error, "An unexpected error occurred during the AI request.");

      reportClientError("Failed to fetch AI response.");
      if (aiRequestIdRef.current === requestId) {
        setAiResponse(`AI request failed: ${message}`);
      }
    } finally {
      if (aiRequestIdRef.current === requestId) {
        setAiLoading(false);
      }
    }
  }

  async function handleSaveToken() {
    try {
      setErrorMessage("");
      await saveAuthSession(authTokenInput);
      setAuthTokenInput("");
      setIsAuthenticated(true);
      setAuthPanelOpen(false);
    } catch (error) {
      const message = formatError(error, "Unable to save login session.");

      reportClientError("Failed to save auth session.");
      setErrorMessage(message);
    }
  }

  async function handleLogout() {
    try {
      setErrorMessage("");
      await clearAuthSession();
      setIsAuthenticated(false);
      setAuthTokenInput("");
      setAuthPanelOpen(true);
    } catch (error) {
      const message = formatError(error, "Unable to clear login session.");

      reportClientError("Failed to clear auth session.");
      setErrorMessage(message);
    }
  }

  useEffect(() => {
    let isDisposed = false;

    void getAuthSession()
      .then((session) => {
        if (isDisposed) {
          return;
        }

        setIsAuthenticated(Boolean(session?.access_token));
      })
      .catch((error) => {
        const message = formatError(error, "Unable to load login session.");

        reportClientError("Failed to load auth session.");
        if (!isDisposed) {
          setErrorMessage(message);
        }
      });

    return () => {
      isDisposed = true;
    };
  }, []);

  useEffect(() => {
    aiLoadingRef.current = aiLoading;
  }, [aiLoading]);

  useEffect(() => {
    let isDisposed = false;
    let unlisten: (() => void) | undefined;

    void listen<CapturedContextEvent>("context-captured", (event) => {
      if (isDisposed) {
        return;
      }

      const capturedText = event.payload.text?.trim();

      setLoading(false);
      setActiveTab("chat");
      setAiResponse("");
      setAiLoading(false);
      aiRequestIdRef.current += 1;

      if (capturedText) {
        setChatPrompt(capturedText);
        userEditedPromptRef.current = false;
      }

      setErrorMessage(event.payload.error ?? "");
    })
      .then((cleanup) => {
        if (isDisposed) {
          cleanup();
          return;
        }

        unlisten = cleanup;
      })
      .catch(() => {
        reportClientError("Failed to register captured context listener.");
      });

    return () => {
      isDisposed = true;
      unlisten?.();
    };
  }, []);

  const hasChatText = chatPrompt.trim().length > 0;

  return (
    <main className="overlay-container">
      <section className="glass-panel" aria-live="polite">
        <header className="panel-header">
          <div className="brand-block">
            <h1>Instant</h1>
          </div>
          <nav className="header-nav" role="tablist" aria-label="AI panels">
            <button
              className={`nav-tab${activeTab === "chat" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeTab === "chat"}
              onClick={() => setActiveTab("chat")}
            >
              Chat
            </button>
            <button
              className={`nav-tab${activeTab === "response" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeTab === "response"}
              onClick={() => setActiveTab("response")}
            >
              Response
              {aiLoading || aiResponse.trim().length > 0 ? (
                <span className="tab-dot" aria-hidden="true" />
              ) : null}
            </button>
          </nav>
          <button
            className={`auth-btn${isAuthenticated ? " connected" : ""}`}
            type="button"
            onClick={() => setAuthPanelOpen((current) => !current)}
            aria-label={isAuthenticated ? "Manage login session" : "Connect login session"}
            title={isAuthenticated ? "Connected" : "Connect"}
          >
            {isAuthenticated ? "●" : "○"}
          </button>
          <button
            className="refresh-btn"
            type="button"
            onClick={() => void fetchClipboardText({ force: true })}
            disabled={loading}
            aria-label="Refresh clipboard context"
            title="Refresh clipboard context"
          >
            ↻
          </button>
        </header>

        <div className="content-area">
          {authPanelOpen ? (
            <section className="auth-panel" aria-label="Login session">
              {isAuthenticated ? (
                <>
                  <span className="auth-status">Connected</span>
                  <button className="auth-action-btn" type="button" onClick={() => void handleLogout()}>
                    Logout
                  </button>
                </>
              ) : (
                <>
                  <input
                    className="auth-token-input"
                    value={authTokenInput}
                    onChange={(event) => setAuthTokenInput(event.currentTarget.value)}
                    placeholder="Paste login token"
                    aria-label="Login token"
                  />
                  <button
                    className="auth-action-btn"
                    type="button"
                    onClick={() => void handleSaveToken()}
                    disabled={!authTokenInput.trim()}
                  >
                    Save
                  </button>
                </>
              )}
            </section>
          ) : null}

          {loading ? (
            <p className="status-text">Reading clipboard...</p>
          ) : errorMessage ? (
            <div className="error-box" role="alert">
              {errorMessage}
            </div>
          ) : null}

          <section className="chat-section">
            {activeTab === "chat" ? (
              <div className="chat-panel" role="tabpanel">
                <textarea
                  className="chat-input"
                  value={chatPrompt}
                  onChange={(event) => {
                    userEditedPromptRef.current = true;
                    setChatPrompt(event.currentTarget.value);
                  }}
                  placeholder="Clipboard text will appear here. Edit it or add your question..."
                  aria-label="Chat prompt with captured clipboard text"
                />
                <button
                  className="send-btn"
                  type="button"
                  onClick={() => void handleAskAI()}
                  aria-label="Send to AI"
                  title="Send to AI"
                  disabled={!hasChatText || aiLoading}
                >
                  ➤
                </button>
              </div>
            ) : (
              <div className="response-panel" role="tabpanel">
                <Suspense
                  fallback={
                    <section className="response-container response-container-loading">
                      <div className="response-spinner" aria-hidden="true" />
                      <p>Preparing response viewer...</p>
                    </section>
                  }
                >
                  <ResponseArea responseText={aiResponse} isLoading={aiLoading} />
                </Suspense>
              </div>
            )}
          </section>
        </div>
      </section>
    </main>
  );
}

export default App;
