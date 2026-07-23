export interface User {
  id: string
  username: string
  displayName: string
  email: string
}

export const currentUser: User = {
  id: 'user-1',
  username: 'me',
  displayName: 'Current User',
  email: 'me@example.local',
}

export async function login(_username: string, _password: string): Promise<User> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  return { ...currentUser }
}

export async function register(_input: { username: string; displayName: string; email: string; password: string }): Promise<User> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  return { ...currentUser }
}
