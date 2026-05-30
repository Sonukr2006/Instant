import { invoke } from "@tauri-apps/api/core";

export type AuthSession = {
  access_token: string;
};

export async function getAuthSession(): Promise<AuthSession | null> {
  return await invoke<AuthSession | null>("get_auth_session");
}

export async function saveAuthSession(accessToken: string): Promise<AuthSession> {
  const trimmedToken = accessToken.trim();

  if (!trimmedToken) {
    throw new Error("Auth token cannot be empty.");
  }

  return await invoke<AuthSession>("save_auth_session", {
    accessToken: trimmedToken,
  });
}

export async function clearAuthSession(): Promise<void> {
  await invoke("clear_auth_session");
}
