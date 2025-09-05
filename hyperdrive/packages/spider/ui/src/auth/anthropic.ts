import { ApiError, exchange_oauth_token as exchangeOauthToken, refresh_oauth_token as refreshOauthToken } from '@caller-utils';

export namespace AuthAnthropic {
  const CLIENT_ID = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

  // Generate PKCE challenge and verifier
  async function generatePKCE() {
    const generateRandomString = (length: number) => {
      const array = new Uint8Array(length);
      crypto.getRandomValues(array);
      return btoa(String.fromCharCode(...array))
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=/g, '');
    };

    const verifier = generateRandomString(32);

    const encoder = new TextEncoder();
    const data = encoder.encode(verifier);
    const hash = await crypto.subtle.digest('SHA-256', data);

    const challenge = btoa(String.fromCharCode(...new Uint8Array(hash)))
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=/g, '');

    return { verifier, challenge };
  }

  export async function authorize() {
    const pkce = await generatePKCE();

    const url = new URL("https://claude.ai/oauth/authorize");
    url.searchParams.set("code", "true");
    url.searchParams.set("client_id", CLIENT_ID);
    url.searchParams.set("response_type", "code");
    url.searchParams.set("redirect_uri", "https://console.anthropic.com/oauth/code/callback");
    url.searchParams.set("scope", "org:create_api_key user:profile user:inference");
    url.searchParams.set("code_challenge", pkce.challenge);
    url.searchParams.set("code_challenge_method", "S256");
    url.searchParams.set("state", pkce.verifier);

    return {
      url: url.toString(),
      verifier: pkce.verifier,
    };
  }

  export async function exchange(code: string, verifier: string) {
    try {
      // Use the backend proxy to avoid CORS issues
      const result = await exchangeOauthToken({
        code: code,
        verifier: verifier,
      });

      return result;
    } catch (error) {
      if (error instanceof ApiError) {
        throw error;
      }
      throw new ExchangeFailed();
    }
  }

  export async function refresh(refreshToken: string) {
    try {
      // Use the backend proxy to avoid CORS issues
      const result = await refreshOauthToken({
        refreshToken: refreshToken,
      });

      return result;
    } catch (error) {
      throw new Error("Failed to refresh token");
    }
  }

  export class ExchangeFailed extends Error {
    constructor() {
      super("Exchange failed");
    }
  }
}
