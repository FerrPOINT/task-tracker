export interface OAuthTokens {
	access_token: string
	refresh_token: string
	expires_in: number
}

export interface Task TrackerDesktop {
	isDesktop: boolean
	startOAuthLogin: (apiUrl: string) => Promise<void>
	onOAuthTokens: (callback: (tokens: OAuthTokens) => void) => void
	onOAuthError: (callback: (error: string) => void) => void
	refreshToken: (apiUrl: string, refreshToken: string) => Promise<OAuthTokens>
	updateQuickEntryShortcut: (shortcut: string) => void
}

declare global {
	interface Window {
		task-trackerDesktop?: Task TrackerDesktop
	}
}
