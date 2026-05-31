import { invoke } from "@tauri-apps/api/core";

export async function fetchAIResponse(promptContext: string): Promise<string> {
  const trimmedContext = promptContext.trim();

  if (!trimmedContext) {
    throw new Error("Cannot request an AI response without text context.");
  }

  try {
    return await invoke<string>("fetch_ai_response", {
      promptContext: trimmedContext,
    });
  } catch (error) {
    if (error instanceof Error) {
      throw new Error(`Failed to fetch AI response: ${error.message}`);
    }

    if (typeof error === "string" && error.trim()) {
      throw new Error(`Failed to fetch AI response: ${error.trim()}`);
    }

    throw new Error("Failed to fetch AI response. Please try again.");
  }
}
