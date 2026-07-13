import type {OAuthTokens} from '@/types/desktop'

export function isDesktopApp(): boolean {
	return !!window.task-trackerDesktop?.isDesktop
}

export function startDesktopOAuthLogin(apiUrl: string): Promise<void> {
	return window.task-trackerDesktop!.startOAuthLogin(apiUrl)
}

export function listenForDesktopOAuthTokens(callback: (tokens: OAuthTokens) => void): void {
	window.task-trackerDesktop!.onOAuthTokens(callback)
}

export function listenForDesktopOAuthError(callback: (error: string) => void): void {
	window.task-trackerDesktop!.onOAuthError(callback)
}

export function refreshDesktopToken(apiUrl: string, refreshToken: string): Promise<OAuthTokens> {
	return window.task-trackerDesktop!.refreshToken(apiUrl, refreshToken)
}
