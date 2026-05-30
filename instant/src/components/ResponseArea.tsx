import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";

export interface ResponseAreaProps {
  responseText: string;
  isLoading: boolean;
}

function ResponseArea({ responseText, isLoading }: ResponseAreaProps) {
  if (isLoading) {
    return (
      <section
        className="response-container response-container-loading"
        aria-live="polite"
      >
        <div className="response-spinner" aria-hidden="true" />
        <p>AI is processing the context...</p>
      </section>
    );
  }

  if (!responseText.trim()) {
    return (
      <section className="response-container response-container-empty">
        AI answer will appear here.
      </section>
    );
  }

  return (
    <section className="response-container" aria-label="AI response">
      <ReactMarkdown
        components={{
          code({ className, children, ...props }) {
            const languageMatch = /language-(\w+)/.exec(className ?? "");
            const language = languageMatch?.[1];
            const code = String(children).replace(/\n$/, "");

            if (!language) {
              return (
                <code className="markdown-inline-code" {...props}>
                  {children}
                </code>
              );
            }

            return (
              <SyntaxHighlighter
                PreTag="div"
                className="markdown-code-block"
                language={language}
                wrapLongLines
                useInlineStyles={false}
              >
                {code}
              </SyntaxHighlighter>
            );
          },
        }}
      >
        {responseText}
      </ReactMarkdown>
    </section>
  );
}

export default ResponseArea;
