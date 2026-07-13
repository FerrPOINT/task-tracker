import type {OAuthTokens} from '@/types/desktop'

export function isDesktopApp(): boolean {
	return false
}

export function startDesktopOAuthLogin(apiUrl: string): Promise<void> {
	throw new Error('Desktop OAuth login is not available')
}

export function listenForDesktopOAuthTokens(callback: (tokens: OAuthTokens) => void): void {
	// no-op
}

export function listenForDesktopOAuthError(callback: (error: string) => void): void {
	// no-op
}

export function refreshDesktopToken(apiUrl: string, refreshToken: string): Promise<OAuthTokens> {
	throw new Error('Desktop token refresh is not available')
}
